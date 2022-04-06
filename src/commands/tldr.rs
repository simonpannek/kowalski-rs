use rust_bert::pipelines::summarization::SummarizationModel;
use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{
    config::{Command, Config},
    error::ExecutionError,
    strings::{ERR_DATA_ACCESS, ERR_MODEL_RUN},
    utils::send_response,
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), ExecutionError> {
    // Get config
    let config = {
        let data = ctx.data.read().await;

        data.get::<Config>().expect(ERR_DATA_ACCESS).clone()
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
        .map(|message| format!("{}: {}", message.author.name, message.content))
        .fold(String::new(), |acc, mut string| {
            string.push('\n');
            string.push_str(&acc);
            string
        });

    let model = SummarizationModel::new(Default::default())?;

    send_response(&ctx, &command, command_config, "Pong!", "I am listening 🐧").await
}
