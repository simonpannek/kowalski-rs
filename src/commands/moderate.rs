use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{
    config::Command,
    database::client::Database,
    error::ExecutionError,
    strings::{ERR_API_LOAD, ERR_CMD_ARGS_INVALID, ERR_DATA_ACCESS},
    utils::{parse_arg, send_response},
};

enum Moderation {
    Pin,
    Delete,
}

impl Display for Moderation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Moderation::Pin => "Auto-pin",
            Moderation::Delete => "Auto-delete",
        };

        write!(f, "{}", name)
    }
}

impl FromStr for Moderation {
    type Err = ExecutionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pin" => Ok(Moderation::Pin),
            "delete" => Ok(Moderation::Delete),
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

    // Parse first argument
    let moderation = Moderation::from_str(parse_arg(options, 0)?)?;

    // Get guild id
    let guild_id = i64::from(command.guild_id.ok_or(ExecutionError::new(ERR_API_LOAD))?);

    let title = format!("{} message", moderation);

    if options.len() > 1 {
        // Parse second argument
        let score: i64 = parse_arg(options, 1)?;

        // Insert or update entry
        match moderation {
            Moderation::Pin => {
                database
                    .client
                    .execute(
                        "
                        INSERT INTO score_auto_pin VALUES ($1::BIGINT, $2::BIGINT)
                        ON CONFLICT (guild) DO UPDATE SET score = $2::BIGINT
                        ",
                        &[&guild_id, &score],
                    )
                    .await?;
            }
            Moderation::Delete => {
                database
                    .client
                    .execute(
                        "
                        INSERT INTO score_auto_delete VALUES ($1::BIGINT, $2::BIGINT)
                        ON CONFLICT (guild) DO UPDATE SET score = $2::BIGINT
                        ",
                        &[&guild_id, &score],
                    )
                    .await?;
            }
        }

        send_response(
            &ctx,
            &command,
            command_config,
            &title,
            &format!(
                "Moderation tool '{}' is now enabled at a score of {}.",
                moderation, score
            ),
        )
        .await
    } else {
        // Delete moderation
        match moderation {
            Moderation::Pin => {
                database
                    .client
                    .execute(
                        "
                        DELETE FROM score_auto_pin
                        WHERE guild = $1::BIGINT
                        ",
                        &[&guild_id],
                    )
                    .await?;
            }
            Moderation::Delete => {
                database
                    .client
                    .execute(
                        "
                        DELETE FROM score_auto_delete
                        WHERE guild = $1::BIGINT
                        ",
                        &[&guild_id],
                    )
                    .await?;
            }
        }

        send_response(
            &ctx,
            &command,
            command_config,
            &title,
            &format!("Moderation tool '{}' is now disabled.", moderation),
        )
        .await
    }
}
