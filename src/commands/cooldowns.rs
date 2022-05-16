use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{config::Command, error::KowalskiError};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    todo!()
}
