use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use serenity::{
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue::Channel,
    },
};

use crate::{
    config::Command,
    database::client::Database,
    error::ExecutionError,
    strings::{ERR_API_LOAD, ERR_CMD_ARGS_INVALID, ERR_DATA_ACCESS},
    utils::{parse_arg, parse_arg_resolved, send_response},
};

enum Action {
    Add,
    Remove,
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Action::Add => "Add",
            Action::Remove => "Remove",
        };

        write!(f, "{}", name)
    }
}

impl FromStr for Action {
    type Err = ExecutionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "add" => Ok(Action::Add),
            "remove" => Ok(Action::Remove),
            "remove all" => Ok(Action::RemoveAll),
            _ => Err(ExecutionError::new(&format!(
                "{}: {}",
                ERR_CMD_ARGS_INVALID, s
            ))),
        }
    }
}

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), ExecutionError> {
    // Get database
    let database = {
        let data = ctx.data.read().await;

        data.get::<Database>().expect(ERR_DATA_ACCESS).clone()
    };

    let options = &command.data.options;

    // Parse arguments
    let action = Action::from_str(parse_arg(options, 0)?)?;
    let channel = match parse_arg_resolved(options, 1)? {
        Channel(channel) => Ok(channel),
        _ => Err(ExecutionError::new(ERR_API_LOAD)),
    }?;

    // Get channel id
    let channel_id = i64::from(channel.id);

    let title = format!("{} drops for channel {}", action, channel.name);

    println!("{}", action);
    println!("{:?}", channel);

    send_response(&ctx, &command, command_config, "Pong!", "I am listening 🐧").await
}
