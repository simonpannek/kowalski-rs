use std::ops::Div;
use std::sync::Arc;

use itertools::Itertools;
use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};
use tokio::task::JoinError;

use crate::{
    config::{Command, Config},
    error::ExecutionError,
    model::Model,
    strings::ERR_DATA_ACCESS,
    utils::send_response,
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), ExecutionError> {
    // Get config and model
    let (config, model) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().expect(ERR_DATA_ACCESS).clone();
        let model = data.get::<Model>().expect(ERR_DATA_ACCESS).clone();

        (config, model)
    };

    // Get messages to analyze
    let messages = command
        .channel_id
        .messages(&ctx.http, |builder| {
            builder.limit(config.general.nlp_max_messages)
        })
        .await?;

    let messages = messages
        .iter()
        .rev()
        .filter(|message| !message.content.is_empty())
        .enumerate()
        .group_by(|(i, _)| i.div(config.general.nlp_group_size))
        .into_iter()
        .map(|(_, messages)| {
            messages
                .map(|(_, message)| {
                    format!(
                        "{}: {}",
                        message.author.name,
                        message
                            .content
                            .chars()
                            .filter(|&char| char != ':')
                            .take(config.general.nlp_max_message_length)
                            .join("")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>();

    let mut summarization = String::new();

    for message in messages {
        let result = analyze(model.clone(), message)
            .await
            .map_err(|why| ExecutionError::new(&format!("{}", why)))?
            .first()
            .cloned()
            .unwrap_or_default();

        summarization.push_str(&result);
        summarization.push('\n');

        send_response(
            &ctx,
            &command,
            command_config,
            "Tl;dr",
            &format!("{}...", summarization),
        )
        .await?;
    }

    send_response(&ctx, &command, command_config, "Tl;dr", &summarization).await
}

async fn analyze(model: Arc<Model>, message: String) -> Result<Vec<String>, JoinError> {
    tokio::task::spawn_blocking(move || {
        let model = model.summarization.lock().expect(ERR_DATA_ACCESS);

        model.summarize(&vec![message])
    })
    .await
}
