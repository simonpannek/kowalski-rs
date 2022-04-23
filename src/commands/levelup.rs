use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use serenity::{
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue::Role,
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
    let action = Action::from_str(parse_arg(options, 0)?)?;
    let role = match parse_arg_resolved(options, 1)? {
        Role(role) => role,
        _ => unreachable!(),
    };
    let score: i64 = parse_arg(options, 2)?;

    // Get guild and role ids
    let guild_db_id = database.get_guild(role.guild_id).await?;
    let role_db_id = database.get_role(role.guild_id, role.id).await?;

    let title = format!("{} level-up role for {}", action, role.name);

    match action {
        Action::Add => {
            database
                .client
                .execute(
                    "
            INSERT INTO score_roles
            VALUES ($1::BIGINT, $2::BIGINT, $3::BIGINT)
            ",
                    &[&guild_db_id, &role_db_id, &score],
                )
                .await?;

            send_response(
                &ctx,
                &command,
                command_config,
                &title,
                &format!(
                    "Users reaching a score of {} will now receive the role {}.",
                    score,
                    role.mention()
                ),
            )
            .await
        }
        Action::Remove => {
            let modified = database
                .client
                .execute(
                    "
            DELETE FROM score_roles
            WHERE guild = $1::BIGINT AND role = $2::BIGINT AND score = $3::BIGINT
            ",
                    &[&guild_db_id, &role_db_id, &score],
                )
                .await?;

            if modified == 0 {
                send_response(
                    &ctx,
                    &command,
                    command_config,
                    &title,
                    &format!(
                        "There is no level-up role defined for role {} score {}.
                        I didn't remove anything.",
                        role.mention(),
                        score
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
                        "I have removed the level-up role {} on score {}.",
                        role.mention(),
                        score
                    ),
                )
                .await
            }
        }
    }
}
