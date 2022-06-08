use std::sync::Arc;

use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};
use tokio::task::JoinError;

use crate::{
    config::{Command, Config},
    data,
    error::KowalskiError,
    model::Model,
    utils::{get_relevant_messages, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get config and model
    let (config, model) = data!(ctx, (Config, Model));

    let messages = get_relevant_messages(ctx, &config, command.channel_id, None).await?;

    let mut summarization = String::new();

    for message in messages {
        let result = analyze(model.clone(), message)
            .await
            .unwrap()
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
        let model = model.summarization.lock().unwrap();

        model.summarize(&vec![message])
    })
    .await
}
