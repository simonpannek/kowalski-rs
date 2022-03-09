use std::str::FromStr;

use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::utils::send_response_complex;
use crate::{
    config::Config,
    config::{Command, Module},
    database::{client::Database, types::ModuleStatus},
    error::ExecutionError,
    strings::{ERR_API_LOAD, ERR_DATA_ACCESS},
    utils::{parse_arg, send_response},
};

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

    // Parse argument
    let module = Module::from_str(parse_arg(options, 0)?)?;

    // Get guild status
    let guild = command.guild_id.ok_or(ExecutionError::new(ERR_API_LOAD))?;
    let status = {
        let row = database
            .client
            .query_opt(
                "SELECT status FROM modules WHERE guild = $1::BIGINT",
                &[&i64::from(guild)],
            )
            .await?;

        row.map_or(ModuleStatus::default(), |row| row.get(0))
    };

    // Get description of module
    let content = match module {
        Module::Owner => "The owner module includes all commands that can be executed by the owner. \
        If no bot owner is on the server or they should not be able to execute owner commands here, \
        this module should be disabled.",
        Module::Utility => "The utility module includes commands that provide commands not associated \
        with any other modules but may be useful for moderation. Utility commands are common commands, \
        often implemented by other bots as well. To avoid duplication, this module can be disabled when \
        required.",
        Module::Score => "The score module provides everything associated with the level-up system of \
        the bot. This includes commands for managing the level-up roles and commands to query the scores \
        and rankings of users. When the module is disabled, no reactions will get tracked as up- or \
        downvotes.",
        Module::ReactionRoles => "The reaction-roles module provides a reaction-role system. A reaction-\
        role binds an emoji and a message to a set of roles. When an user react to this message, the \
        bot will assign the defined set of roles to them. You can also limit a reaction-role. In this \
        case, the bot will only assign the reaction-role to users as long as there are slots available.",
    };

    send_response_complex(
        &ctx,
        &command,
        command_config,
        &format!("Information about module '{:?}'", module),
        content,
        |embed| embed,
        Vec::new(),
    )
    .await
}
