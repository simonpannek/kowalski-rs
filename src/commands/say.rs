use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{
    config::{Command, Config},
    error::ExecutionError,
    history::History,
    strings::ERR_DATA_ACCESS,
    utils::{parse_arg, parse_arg_name, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), ExecutionError> {
    // Get config and lock to history
    let (config, history_lock) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().expect(ERR_DATA_ACCESS).clone();
        let history_lock = data.get::<History>().expect(ERR_DATA_ACCESS).clone();

        (config, history_lock)
    };

    let options = &command.data.options;

    // Parse arguments
    let title_name = parse_arg_name(options, 0)?;
    let title = parse_arg(options, 0)?;
    let content_name = parse_arg_name(options, 1)?;
    let content = parse_arg(options, 1)?;

    // Add title and content to history
    {
        let mut history = history_lock.write().await;

        history.add_entry(&config, command.user.id, title_name, title);
        history.add_entry(&config, command.user.id, content_name, content);
    }

    send_response(&ctx, &command, command_config, title, content).await
}
