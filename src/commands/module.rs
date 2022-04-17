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
    database::{client::Database, types::ModuleStatus},
    error::ExecutionError,
    strings::{ERR_API_LOAD, ERR_CMD_ARGS_INVALID, ERR_DATA_ACCESS},
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
    type Err = ExecutionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "enable" => Ok(Action::Enable),
            "disable" => Ok(Action::Disable(false)),
            "remove" => Ok(Action::Disable(true)),
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
    // Get config and database
    let (config, database) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().expect(ERR_DATA_ACCESS).clone();
        let database = data.get::<Database>().expect(ERR_DATA_ACCESS).clone();

        (config, database)
    };

    let options = &command.data.options;

    // Parse arguments
    let action = Action::from_str(parse_arg(options, 0)?)?;
    let module = Module::from_str(parse_arg(options, 1)?)?;

    // Get guild status
    let guild = command.guild_id.ok_or(ExecutionError::new(ERR_API_LOAD))?;
    let status: ModuleStatus = {
        let row = database
            .client
            .query_opt(
                "SELECT status FROM modules WHERE guild = $1::BIGINT",
                &[&i64::from(guild)],
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
                        &[&i64::from(guild)],
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
) -> Result<(), ExecutionError> {
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
) -> Result<(), ExecutionError> {
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
            &[&status, &i64::from(guild)],
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
