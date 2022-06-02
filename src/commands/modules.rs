use std::str::FromStr;

use itertools::Itertools;
use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};
use strum::IntoEnumIterator;

use crate::utils::send_response;
use crate::{
    config::{Command, Config, Module},
    data,
    database::{client::Database, types::ModuleStatus},
    error::KowalskiError,
    utils::{parse_arg, send_response_complex},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get database
    let database = data!(ctx, Database);

    let guild_id = command.guild_id.unwrap();

    // Get guild id
    let guild_db_id = database.get_guild(guild_id).await?;

    let status = database
        .client
        .query_opt(
            "
                SELECT status
                FROM modules
                WHERE guild = $1::BIGINT
                ",
            &[&guild_db_id],
        )
        .await?
        .map_or(ModuleStatus::default(), |row| row.get(0));

    let mut fields = Vec::new();

    for module in Module::iter() {
        // Get the description for the current module
        let content = match module {
            Module::Owner => "The owner module includes all commands that can be executed by the owner. \
        If no bot owner is on the server or o owner should not be able to execute owner commands here, \
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
            Module::Analyze => "The analyze module provides commands to analyze previous messages written \
        in a specific channel.",
        };

        // Check whether the current module is enabled
        let enabled = match module {
            Module::Owner => status.owner,
            Module::Utility => status.utility,
            Module::Score => status.score,
            Module::ReactionRoles => status.reaction_roles,
            Module::Analyze => status.analyze,
        };

        fields.push((
            format!(
                "{:?} ({}):",
                module,
                if enabled { "enabled" } else { "disabled" }
            ),
            content,
            false,
        ))
    }

    send_response_complex(
        &ctx,
        &command,
        command_config,
        "Modules",
        "",
        |embed| embed.fields(fields.clone()),
        Vec::new(),
    )
    .await
}
