use rust_bert::pipelines::conversation::ConversationManager;
use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};
use itertools::Itertools;

use crate::{
    config::{Command, Config},
    data,
    error::KowalskiError,
    history::History,
    model::Model,
    utils::{parse_arg, parse_arg_name, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get config, lock to history and model
    let (config, history_lock, model) = data!(ctx, (Config, History, Model));

    let options = &command.data.options;

    // Parse argument
    let question = parse_arg::<String>(options, 0)?;

    // Add question to history
    {
        let mut history = history_lock.write().await;

        history.add_entry(
            &config,
            command.user.id,
            parse_arg_name(options, 0)?,
            &question,
        );
    }

    // Get messages to analyze
    let messages = command
        .channel_id
        .messages(&ctx.http, |builder| builder.limit(10))
        .await?;

    let mut messages = messages
        .iter()
        .rev()
        .filter(|message| !message.content.is_empty())
        .map(|message| {
            message
                .content
                .chars()
                .take(config.general.nlp_max_message_length)
                .join("")
        })
        .collect::<Vec<_>>();

    messages.push(question.clone());

    let mut result = tokio::task::spawn_blocking(move || {
        // Create conversation
        let mut manager = ConversationManager::new();
        let conversation_id = manager.create_empty();
        let conversation = manager.get(&conversation_id).unwrap();

        let sliced = {
            // Skip one message if the count is even
            let skip = (messages.len() + 1) % 2;

            messages
                .iter()
                .skip(skip)
                .map(|message| (message as &dyn AsRef<str>).as_ref())
                .collect::<Vec<_>>()
        };

        let model = model.conversation.lock().unwrap();

        // Load messages
        let encoded = model.encode_prompts(&sliced);
        conversation.load_from_history(&sliced, &encoded);

        model
            .generate_responses(&mut manager)
            .get(&conversation_id)
            .unwrap()
            .to_string()
    })
    .await
    .unwrap();

    if result.is_empty() {
        result = "I prefer not to answer...".to_string();
    }

    send_response(
        &ctx,
        &command,
        command_config,
        &question,
        &format!("**Oracle:** {}", result),
    )
    .await
}
