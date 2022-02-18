use serenity::{
    client::Context,
    model::interactions::{application_command::ApplicationCommandInteraction, Interaction},
};
use tracing::error;

use crate::strings::ERR_USER_TITLE;
use crate::{
    commands::*,
    config::{CommandType, Config},
    error::ExecutionError,
    strings::{
        ERR_API_LOAD, ERR_CMD_EXECUTION, ERR_CMD_NOT_FOUND, ERR_CMD_PERMISSION, ERR_DATA_ACCESS,
        ERR_USER_EXECUTION_FAILED,
    },
    utils::send_failure,
};

pub async fn interaction_create(ctx: &Context, interaction: Interaction) {
    if let Interaction::ApplicationCommand(command) = interaction {
        if let Err(why) = execute_command(ctx, &command).await {
            send_failure(ctx, &command, ERR_USER_TITLE, ERR_USER_EXECUTION_FAILED).await;
            error!("{}: {:?}", ERR_CMD_EXECUTION, why);
        }
    }
}

async fn execute_command(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<(), ExecutionError> {
    // Get config
    let config = {
        let data = ctx.data.read().await;

        data.get::<Config>().expect(ERR_DATA_ACCESS).clone()
    };

    // Get command name
    let name = &command.data.name;
    // Get command config
    let command_config = config
        .commands
        .get(name)
        .ok_or(ExecutionError::new(ERR_CMD_NOT_FOUND))?;

    // Check for permissions (this should not be necessary, just an additional fallback)
    if !command_config.default_permission {
        let mut can_execute = false;

        // Check if the user is a owner if the comment requires it to be one
        if command_config.owner.unwrap_or_default() {
            let owners = &config.general.owners;
            if owners.contains(&u64::from(command.user.id)) {
                can_execute = true;
            }
        }

        // Check if the user has the required permissions if there are any
        if let Some(permission) = command_config.permission {
            if let Some(member) = &command.member {
                // Get permissions of the user
                let permissions = member
                    .permissions
                    .ok_or(ExecutionError::new(ERR_API_LOAD))?;

                // Check whether the user has sufficient permissions
                can_execute = permissions.contains(permission);
            }
        }

        // Fail if user cannot execute the command
        if !can_execute {
            return Err(ExecutionError::new(ERR_CMD_PERMISSION));
        }
    }

    // Execute the command
    match command_config.command_type {
        CommandType::Ping => ping::execute(ctx, command, command_config).await,
        CommandType::About => about::execute(ctx, command, command_config).await,
        CommandType::Module => module::execute(ctx, command, command_config).await,
        CommandType::Sql => sql::execute(ctx, command, command_config).await,
    }
}
