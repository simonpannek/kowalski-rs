use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{config::Command, error::ExecutionError};

pub async fn execute(
    _ctx: &Context,
    _command: &ApplicationCommandInteraction,
    _command_config: &Command,
) -> Result<(), ExecutionError> {
    todo!()
}
