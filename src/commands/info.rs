use std::str::FromStr;

use itertools::Itertools;
use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

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
    // Get config and database
    let (config, database) = data!(ctx, (Config, Database));

    let options = &command.data.options;

    // Parse argument
    let module = Module::from_str(parse_arg(options, 0)?)?;

    let guild_id = command.guild_id.unwrap();

    // Get guild ids
    let guild_db_id = database.get_guild(guild_id).await?;

    let status = {
        let row = database
            .client
            .query_opt(
                "
                SELECT status
                FROM modules
                WHERE guild = $1::BIGINT
                ",
                &[&guild_db_id],
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
        Module::Analyze => "The analyze module provides commands to analyze previous messages written \
        in a specific channel.",
    };

    let fields = {
        let mut fields = Vec::new();

        let enabled = match module {
            Module::Owner => status.owner,
            Module::Utility => status.utility,
            Module::Score => status.score,
            Module::ReactionRoles => status.reaction_roles,
            Module::Analyze => status.analyze,
        };

        fields.push((
            "Module status".to_string(),
            format!(
                "The module {:?} is currently {} on this server.",
                module,
                if enabled { "enabled" } else { "disabled" }
            ),
            false,
        ));

        let commands = config
            .commands
            .iter()
            .filter(|&(_, command)| {
                command
                    .module
                    .as_ref()
                    .map_or(false, |command_module| command_module == &module)
            })
            .sorted_by(|&(name_a, _), &(name_b, _)| name_a.cmp(name_b))
            .map(|(name, command)| format!("- **{}** ({})", name, command.description))
            .join("\n");

        fields.push(("Commands".to_string(), commands, false));

        fields
    };

    send_response_complex(
        &ctx,
        &command,
        command_config,
        &format!("Information about module '{:?}'", module),
        content,
        |embed| embed.fields(fields.clone()),
        Vec::new(),
    )
    .await
}
