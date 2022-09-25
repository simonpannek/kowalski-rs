use std::{
    fmt::{Display, Formatter},
    str::FromStr,
    time::Duration,
};

use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};
use tokio::sync::Mutex;

use crate::{
    config::{Command, Config, Module},
    data,
    database::{client::Database, types::ModuleStatus},
    error::KowalskiError,
    error::KowalskiError::DiscordApiError,
    strings::ERR_CMD_ARGS_INVALID,
    utils::{
        create_module_command, parse_arg, send_confirmation, send_failure, send_response,
        InteractionResponse,
    },
};

/// Lock to avoid race conditions when modifying the status object
static LOCK: Mutex<i32> = Mutex::const_new(1);

enum Action {
    Enable,
    Disable(bool),
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Action::Enable => "Enable",
            Action::Disable(remove) => {
                if *remove {
                    "Remove"
                } else {
                    "Disable"
                }
            }
        };

        write!(f, "{}", name)
    }
}

impl FromStr for Action {
    type Err = KowalskiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "enable" => Ok(Action::Enable),
            "disable" => Ok(Action::Disable(false)),
            "remove" => Ok(Action::Disable(true)),
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
    let module = Module::from_str(parse_arg(options, 1)?).unwrap();

    // Disable the module in private channels
    if matches!(command.guild_id, None) {
        send_failure(
            &ctx,
            command,
            "Command not available",
            "Module commands are only available on guilds.",
        )
        .await;

        return Ok(());
    }

    // Only allow the owner to enable/disable the owner module
    if matches!(module, Module::Owner) {
        if !config.general.owners.contains(&command.user.id.0) {
            send_failure(
                &ctx,
                command,
                "Insufficient permissions",
                "I'm sorry, but this module is restricted.",
            )
            .await;

            return Ok(());
        }
    }

    let guild_id = command.guild_id.unwrap();

    // Get guild id
    let guild_db_id = database.get_guild(guild_id).await?;

    // Get the status, modify it and update it in the database if necessary
    let status: Option<ModuleStatus> = {
        let _mutex = LOCK.lock().await;

        // Get current guild status
        let status: ModuleStatus = {
            let row = database
                .client
                .query_opt(
                    "SELECT status FROM modules WHERE guild = $1::BIGINT",
                    &[&guild_db_id],
                )
                .await?;

            match row {
                Some(row) => row.get(0),
                None => {
                    database
                        .client
                        .execute(
                            "
                        INSERT INTO modules
                        VALUES ($1::BIGINT, B'00000000')
                        ",
                            &[&guild_db_id],
                        )
                        .await?;

                    ModuleStatus::default()
                }
            }
        };

        // Copy status to compare it to the old status later
        let mut status_new = status.clone();

        // Update the status object
        let enable = matches!(action, Action::Enable);
        match module {
            Module::Owner => status_new.owner = enable,
            Module::Utility => status_new.utility = enable,
            Module::Score => status_new.score = enable,
            Module::ReactionRoles => status_new.reaction_roles = enable,
            Module::Analyze => status_new.analyze = enable,
        };

        // Check whether the status has changed
        if status != status_new {
            // Update the object in the database so we can drop the lock

            // Get guild id
            let guild_db_id = database.get_guild(guild_id).await?;

            // Update the database entry
            database
                .client
                .execute(
                    "
            UPDATE modules
            SET status = $1::BIT(8)
            WHERE guild = $2::BIGINT
            ",
                    &[&status_new, &guild_db_id],
                )
                .await?;

            Some(status_new)
        } else {
            None
        }
    };

    // Get title of the embed
    let title = format!("{} module '{:?}'", action, module);

    match action {
        Action::Disable(true) => {
            // Check for the interaction response
            let response = send_confirmation(
                ctx,
                command,
                command_config,
                &format!("Are you really sure you want to remove all of the module data provided by the module '{:?}'?
                This cannot be reversed, all data will be gone permanently!", module),
                Duration::from_secs(config.general.interaction_timeout),
            )
            .await?;

            match response {
                Some(InteractionResponse::Continue) => {
                    remove(ctx, command, command_config, title, module).await
                }
                Some(InteractionResponse::Abort) => {
                    send_response(ctx, command, command_config, &title, "Aborted the action.").await
                }
                None => Ok(()),
            }
        }
        _ => {
            // Enable/disable the module
            match status {
                Some(status) => {
                    send_response(
                        ctx,
                        command,
                        command_config,
                        &title,
                        "I'm updating the module... This can take some time.",
                    )
                    .await?;

                    // Update the guild commands
                    create_module_command(ctx, &config, guild_id, &status).await;

                    send_response(
                        ctx,
                        command,
                        command_config,
                        &title,
                        "I have updated the module.",
                    )
                    .await
                }
                None => {
                    // No real update
                    send_response(
                        ctx,
                        command,
                        command_config,
                        &title,
                        "The state of the module did not change. No need to update anything.",
                    )
                    .await
                }
            }
        }
    }
}

async fn remove(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
    title: String,
    module: Module,
) -> Result<(), KowalskiError> {
    // Get database
    let database = data!(ctx, Database);

    let guild_id = command.guild_id.unwrap();

    // Get guild id
    let guild_db_id = guild_id.0 as i64;

    match module {
        Module::Utility => {
            database
                .client
                .execute(
                    "DELETE FROM reminders WHERE guild = $1::BIGINT",
                    &[&guild_db_id],
                )
                .await?;
        }
        Module::Score => {
            database
                .client
                .execute(
                    "
                    DELETE FROM score_auto_delete WHERE guild = $1::BIGINT;

                    DELETE FROM score_auto_pin WHERE guild = $1::BIGINT;

                    DELETE FROM score_cooldowns WHERE guild = $1::BIGINT;

                    DELETE FROM score_drops WHERE guild = $1::BIGINT;

                    DELETE FROM score_emojis WHERE guild = $1::BIGINT;

                    DELETE FROM score_roles WHERE guild = $1::BIGINT;
                    ",
                    &[&guild_db_id],
                )
                .await?;
        }
        Module::ReactionRoles => {
            database
                .client
                .execute(
                    "DELETE FROM reminders WHERE guild = $1::BIGINT",
                    &[&guild_db_id],
                )
                .await?;
        }
        _ => {
            return send_response(
                ctx,
                command,
                command_config,
                &title,
                "I have updated the module. There was no need to remove any data.",
            )
            .await;
        }
    }

    send_response(
        ctx,
        command,
        command_config,
        &title,
        "I have removed all of the module data.",
    )
    .await
}
