use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use serenity::model::id::ChannelId;
use serenity::prelude::Mentionable;
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
    let partial_channel = match parse_arg_resolved(options, 1)? {
        Channel(channel) => Ok(channel),
        _ => Err(ExecutionError::new(ERR_API_LOAD)),
    }?;
    let channel = {
        let id = ChannelId::from(partial_channel.id);
        id.to_channel(&ctx.http).await?
    };

    // Get guild and channel ids
    let guild_id = i64::from(command.guild_id.ok_or(ExecutionError::new(ERR_API_LOAD))?);
    let channel_id = i64::from(partial_channel.id);

    let title = format!("{} drops for channel {}", action, partial_channel.name);

    match action {
        Action::Add => {
            database
                .client
                .execute(
                    "
            INSERT INTO score_drops
            VALUES ($1::BIGINT, $2::BIGINT)
            ",
                    &[&guild_id, &channel_id],
                )
                .await?;

            send_response(
                &ctx,
                &command,
                command_config,
                &title,
                &format!(
                    "Reactions now might drop into channel {} when a user leaves the guild.",
                    channel.mention()
                ),
            )
            .await
        }
        Action::Remove => {
            let modified = database
                .client
                .execute(
                    "
            DELETE FROM score_drops
            WHERE guild = $1::BIGINT AND channel = $2::BIGINT
            ",
                    &[&guild_id, &channel_id],
                )
                .await?;

            if modified == 0 {
                send_response(
                    &ctx,
                    &command,
                    command_config,
                    &title,
                    &format!(
                        "Drops where not activated for channel {}.
                        I didn't remove anything.",
                        channel.mention()
                    ),
                )
                .await
            } else {
                send_response(
                    &ctx,
                    &command,
                    command_config,
                    &title,
                    &format!(
                        "Reactions will no longer drop into channel {} when a user leaves the guild.",
                        channel.mention()
                    ),
                )
                    .await
            }
        }
    }
}
