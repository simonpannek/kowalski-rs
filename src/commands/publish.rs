use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use rand::Rng;
use serenity::{
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue::Channel,
    },
    prelude::Mentionable,
};

use crate::{
    config::{Command, Config},
    data,
    database::client::Database,
    error::KowalskiError,
    error::KowalskiError::DiscordApiError,
    strings::ERR_CMD_ARGS_INVALID,
    utils::{parse_arg, send_response},
};

enum Action {
    Enable,
    Disable,
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Action::Enable => "Enable",
            Action::Disable => "Disable",
        };

        write!(f, "{}", name)
    }
}

impl FromStr for Action {
    type Err = KowalskiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "enable" => Ok(Action::Enable),
            "disable" => Ok(Action::Disable),
            _ => Err(DiscordApiError(ERR_CMD_ARGS_INVALID.to_string())),
        }
    }
}

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get config and database
    let (config, database) = data!(ctx, (Config, Database));

    let options = &command.data.options;

    // Parse arguments
    let action = Action::from_str(parse_arg(options, 0)?).unwrap();

    let guild_id = command.guild_id.unwrap();

    // Get guild id
    let guild_db_id = database.get_guild(guild_id).await?;

    let title = format!("{} publishing of events", action);

    match action {
        Action::Enable => {
            let id = {
                let row = database
                    .client
                    .query_opt(
                        "
                SELECT id FROM publishing
                WHERE guild = $1::BIGINT
            ",
                        &[&guild_db_id],
                    )
                    .await?;

                row.map(|row| row.get::<_, String>(0))
            };

            match id {
                Some(id) => {
                    send_response(
                        &ctx,
                        &command,
                        command_config,
                        &title,
                        &format!(
                            "
                        The calendar is already public. You can find it here:
                        {}/{}
                        ",
                            config.general.publishing_link, id
                        ),
                    )
                    .await
                }
                None => {
                    // Generate a random id
                    let id: String = {
                        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
                        let mut rng = rand::thread_rng();

                        (0..config.general.publishing_length)
                            .map(|_| {
                                let idx = rng.gen_range(0..CHARSET.len());
                                CHARSET[idx] as char
                            })
                            .collect()
                    };

                    database
                        .client
                        .execute(
                            "
                        INSERT INTO publishing
                        VALUES($1::TEXT, $2::BIGINT)
                        ",
                            &[&id, &guild_db_id],
                        )
                        .await?;

                    send_response(
                        &ctx,
                        &command,
                        command_config,
                        &title,
                        &format!(
                            "
                        The calendar is now public and available here:
                        {}/{}
                        ",
                            config.general.publishing_link, id
                        ),
                    )
                    .await
                }
            }
        }
        Action::Disable => {
            database
                .client
                .execute(
                    "
            DELETE FROM publishing
            WHERE guild = $1::BIGINT
            ",
                    &[&guild_db_id],
                )
                .await?;

            send_response(
                &ctx,
                &command,
                command_config,
                &title,
                "The event calendar of this guild is not public anymore.",
            )
            .await
        }
    }
}
