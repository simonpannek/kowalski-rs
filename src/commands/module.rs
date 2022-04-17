use std::{
    fmt::{Display, Formatter},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use serenity::{
    client::Context,
    model::{id::GuildId, interactions::application_command::ApplicationCommandInteraction},
};

use crate::{
    config::{Command, Config, Module},
    data,
    database::{client::Database, types::ModuleStatus},
    error::KowalskiError,
    error::KowalskiError::DiscordApiError,
    strings::ERR_CMD_ARGS_INVALID,
    utils::{
        create_module_command, parse_arg, send_confirmation, send_response, InteractionResponse,
    },
};

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
    let action = Action::from_str(parse_arg(options, 0)?)?;
    let module = Module::from_str(parse_arg(options, 1)?)?;

    // Get guild status
    let guild = command.guild_id.unwrap();
    let status: ModuleStatus = {
        let row = database
            .client
            .query_opt(
                "SELECT status FROM modules WHERE guild = $1::BIGINT",
                &[&(guild.0 as i64)],
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
                        &[&(guild.0 as i64)],
                    )
                    .await?;

                ModuleStatus::default()
            }
        }
    };

    // Copy status to compare it to the old status later
    let mut status_new = status.clone();

    // Update status
    let enable = matches!(action, Action::Enable);
    match module {
        Module::Owner => status_new.owner = enable,
        Module::Utility => status_new.utility = enable,
        Module::Score => status_new.score = enable,
        Module::ReactionRoles => status_new.reaction_roles = enable,
        Module::Analyze => status_new.analyze = enable,
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
                    remove(ctx, command, command_config, title, module, database).await
                }
                Some(InteractionResponse::Abort) => {
                    send_response(ctx, command, command_config, &title, "Aborted the action.").await
                }
                None => Ok(()),
            }
        }
        _ => {
            // Enable/disable the module
            if status == status_new {
                // No real update
                send_response(
                    ctx,
                    command,
                    command_config,
                    &title,
                    "The state of the module did not change. No need to update anything.",
                )
                .await
            } else {
                update(
                    ctx,
                    command,
                    command_config,
                    title,
                    &config,
                    guild,
                    status_new,
                    database,
                )
                .await
            }
        }
    }
}

async fn remove(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
    title: String,
    _module: Module,
    _database: Arc<Database>,
) -> Result<(), KowalskiError> {
    // TODO

    send_response(
        ctx,
        command,
        command_config,
        &title,
        "I have removed all of the module data.",
    )
    .await
}

async fn update(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
    title: String,
    config: &Config,
    guild: GuildId,
    status: ModuleStatus,
    database: Arc<Database>,
) -> Result<(), KowalskiError> {
    // Update the guild commands
    create_module_command(ctx, config, guild, &status).await;

    // Update the database entry
    database
        .client
        .execute(
            "
            UPDATE modules
            SET status = $1::BIT(8)
            WHERE guild = $2::BIGINT
            ",
            &[&status, &(guild.0 as i64)],
        )
        .await?;

    send_response(
        ctx,
        command,
        command_config,
        &title,
        "I have updated the module.",
    )
    .await
}
