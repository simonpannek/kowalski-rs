use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use serenity::{
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue::Channel,
    },
    prelude::Mentionable,
};

use crate::{
    config::Command,
    data,
    database::client::Database,
    error::KowalskiError,
    error::KowalskiError::DiscordApiError,
    strings::ERR_CMD_ARGS_INVALID,
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
    type Err = KowalskiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "add" => Ok(Action::Add),
            "remove" => Ok(Action::Remove),
            _ => Err(DiscordApiError(ERR_CMD_ARGS_INVALID.to_string())),
        }
    }
}

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get database
    let database = data!(ctx, Database);

    let options = &command.data.options;

    // Parse arguments
    let action = Action::from_str(parse_arg(options, 0)?).unwrap();
    let partial_channel = match parse_arg_resolved(options, 1)? {
        Channel(channel) => channel,
        _ => unreachable!(),
    };
    let channel = partial_channel.id.to_channel(&ctx.http).await?;

    let guild_id = command.guild_id.unwrap();

    // Get guild and channel ids
    let guild_db_id = database.get_guild(guild_id).await?;
    let channel_db_id = database.get_channel(guild_id, partial_channel.id).await?;

    let title = format!(
        "{} drops for channel {}",
        action,
        partial_channel.name.as_ref().unwrap()
    );

    match action {
        Action::Add => {
            database
                .client
                .execute(
                    "
            INSERT INTO score_drops
            VALUES($1::BIGINT, $2::BIGINT)
            ON CONFLICT
            DO NOTHING
            ",
                    &[&guild_db_id, &channel_db_id],
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
                    &[&guild_db_id, &channel_db_id],
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
