use itertools::Itertools;
use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};
use std::ops::Div;

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
                        message.content.replace(':', "")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect::<Vec<_>>();

    let result = tokio::task::spawn_blocking(move || {
        let model = model.summarization.lock().expect(ERR_MODEL_RUN);

        model.summarize(&messages)
    })
    .await
    .map_err(|why| ExecutionError::new(&format!("{}", why)))?;

    send_response(&ctx, &command, command_config, "Tl;dr", &result.join("\n")).await
}
