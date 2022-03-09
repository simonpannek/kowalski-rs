use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{
    config::Command,
    error::ExecutionError,
    utils::{parse_arg, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), ExecutionError> {
    let options = &command.data.options;

    // Parse arguments
    let title = parse_arg(options, 0)?;
    let content = parse_arg(options, 1)?;

    send_response(&ctx, &command, command_config, title, content).await
}
