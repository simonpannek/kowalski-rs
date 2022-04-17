use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{config::Command, error::KowalskiError, utils::send_response};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    send_response(&ctx, &command, command_config, "Pong!", "I am listening 🐧").await
}
