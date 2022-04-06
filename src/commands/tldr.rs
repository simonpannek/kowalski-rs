use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{
    config::{Command, Config},
    error::ExecutionError,
    model::Model,
    strings::{ERR_DATA_ACCESS, ERR_MODEL_RUN},
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
        .filter(|message| !message.content.is_empty())
        .map(|message| {
            format!(
                "{}: \"{}\"",
                message.author.name,
                message.content.replace('"', "").replace('#', "")
            )
        })
        .fold(String::new(), |acc, mut string| {
            string.push('\n');
            string.push_str(&acc);
            string
        });

    let result = tokio::task::spawn_blocking(move || {
        let model = model.summarization.lock().expect(ERR_MODEL_RUN);

        model.summarize(&vec![messages])
    })
    .await
    .map_err(|why| ExecutionError::new(&format!("{}", why)))?;

    send_response(
        &ctx,
        &command,
        command_config,
        "Tl;dr",
        result.get(0).ok_or(ExecutionError::new(ERR_MODEL_RUN))?,
    )
    .await
}
