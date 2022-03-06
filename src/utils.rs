use std::{str::FromStr, time::Duration};

use linked_hash_map::LinkedHashMap;
use serde::Deserialize;
use serenity::model::prelude::application_command::ApplicationCommandInteractionDataOptionValue;
use serenity::{
    builder::{
        CreateActionRow, CreateApplicationCommand, CreateApplicationCommandOption, CreateEmbed,
    },
    client::Context,
    model::{
        id::GuildId,
        interactions::{
            application_command::{
                ApplicationCommand, ApplicationCommandInteraction,
                ApplicationCommandInteractionDataOption,
                ApplicationCommandPermissionType::{Role, User},
            },
            message_component::ButtonStyle,
            InteractionResponseType::ChannelMessageWithSource,
        },
        Permissions,
    },
    utils::Colour,
};
use tracing::{error, warn};

use crate::config::{CommandOption, Config, Module, Value};
use crate::database::types::ModuleStatus;
use crate::strings::{ERR_API_LOAD, ERR_CMD_CREATION, ERR_CMD_NOT_FOUND, ERR_CMD_SET_PERMISSION};
use crate::{
    config::Command,
    error::ExecutionError,
    strings::{ERR_CMD_ARGS_INVALID, ERR_CMD_ARGS_LENGTH, ERR_CMD_SEND_FAILURE},
};

pub enum InteractionResponse {
    Continue,
    Abort,
}

impl FromStr for InteractionResponse {
    type Err = ExecutionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "continue" => Ok(InteractionResponse::Continue),
            "abort" => Ok(InteractionResponse::Abort),
            _ => Err(ExecutionError::new(&format!(
                "{}: {}",
                ERR_CMD_ARGS_INVALID, s
            ))),
        }
    }
}

/// Send a embed response, waiting for confirmation.
pub async fn send_confirmation(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
    content: &str,
    timeout: Duration,
) -> Result<Option<InteractionResponse>, ExecutionError> {
    // Create the action row for the interaction
    let mut row = CreateActionRow::default();
    row.create_button(|button| {
        button
            .label("Abort")
            .custom_id("abort")
            .style(ButtonStyle::Secondary)
    })
    .create_button(|button| {
        button
            .label("Continue")
            .custom_id("continue")
            .style(ButtonStyle::Danger)
    });

    // Send the confirmation query
    send_response_complex(
        ctx,
        command,
        command_config,
        "Confirmation",
        content,
        |embed| embed.color(Colour::GOLD),
        vec![row],
    )
    .await?;

    // Get the message
    let message = command.get_interaction_response(&ctx.http).await?;
    // Get the interaction response
    let interaction = message
        .await_component_interaction(&ctx)
        .author_id(u64::from(command.user.id.0))
        .timeout(timeout)
        .await;

    let response = match interaction {
        Some(interaction) => Some(InteractionResponse::from_str(
            interaction.data.custom_id.as_str(),
        )?),
        _ => {
            send_response(
                ctx,
                command,
                command_config,
                "Timed out",
                "You took too long to respond :(",
            )
            .await?;

            None
        }
    };

    Ok(response)
}

/// Edit a simple embed response, only giving the title and content.
pub async fn send_response(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
    title: &str,
    content: &str,
) -> Result<(), ExecutionError> {
    send_response_complex(
        ctx,
        command,
        command_config,
        title,
        content,
        |embed| embed,
        Vec::new(),
    )
    .await
}

/// Edit a embed response, giving the title, content and a function further editing the embed.
pub async fn send_response_complex<F>(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
    title: &str,
    content: &str,
    update: F,
    action_rows: Vec<CreateActionRow>,
) -> Result<(), ExecutionError>
where
    F: Fn(&mut CreateEmbed) -> &mut CreateEmbed,
{
    let mut embed = create_embed(title, content);
    embed.color(Colour::from((47, 49, 54)));

    // Add module to the footer if the command belongs to a module
    if let Some(module) = &command_config.module {
        embed.footer(|footer| footer.text(format!("Module: {:?}", module)));
    }

    // Apply changed by the given function
    update(&mut embed);

    edit_embed(ctx, command, embed, action_rows).await
}

/// Send a failure embed response, giving the title and content.
pub async fn send_failure(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    title: &str,
    content: &str,
) {
    let mut embed = create_embed(title, content);
    embed.color(Colour::RED);

    // If a response exists already, edit the existing message, otherwise, send a new one
    let result = match command.get_interaction_response(&ctx.http).await {
        Ok(_) => edit_embed(ctx, command, embed, Vec::new()).await,
        Err(_) => send_embed(ctx, command, embed, Vec::new()).await,
    };

    // If we have failed once already, we only log the error without notifying the user
    if let Err(why) = result {
        error!("{}: {}", ERR_CMD_SEND_FAILURE, why);
    }
}

fn create_embed(title: &str, content: &str) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed.title(title).description(content);
    embed
}

async fn send_embed(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    embed: CreateEmbed,
    action_rows: Vec<CreateActionRow>,
) -> Result<(), ExecutionError> {
    command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(ChannelMessageWithSource)
                .interaction_response_data(|data| {
                    data.add_embed(embed)
                        .components(|components| components.set_action_rows(action_rows))
                })
        })
        .await
        .map_err(|why| ExecutionError::new(&format!("{}", why)))
}

async fn edit_embed(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    embed: CreateEmbed,
    action_rows: Vec<CreateActionRow>,
) -> Result<(), ExecutionError> {
    command
        .edit_original_interaction_response(&ctx.http, |response| {
            response
                .components(|components| components.set_action_rows(action_rows))
                .add_embed(embed)
        })
        .await
        .map(|_| {})
        .map_err(|why| ExecutionError::new(&format!("{}", why)))
}

// TODO: Avoid rate limiting here

/// Create a general command
pub fn create_command(name: &str, command_config: &Command) -> CreateApplicationCommand {
    let mut command = CreateApplicationCommand::default();

    command
        .name(name)
        .description(&command_config.description)
        .default_permission(command_config.default_permission);

    // Add options if there are any
    if let Some(options) = &command_config.options {
        let (required, unrequired): (
            LinkedHashMap<&String, &CommandOption>,
            LinkedHashMap<&String, &CommandOption>,
        ) = options
            .iter()
            .partition(|(_, option)| option.required.unwrap_or_default());

        // Add required options first
        for (name, option_config) in required {
            let option = create_option(name, option_config);
            command.add_option(option);
        }

        // Add unrequired options afterwards
        for (name, option_config) in unrequired {
            let option = create_option(name, option_config);
            command.add_option(option);
        }
    }

    command
}

/// Create a command belonging to a module.
pub async fn create_module_command(
    ctx: &Context,
    config: &Config,
    guild: GuildId,
    status: &ModuleStatus,
) {
    // Filter commands for the configuration of the current guild
    let filtered = config
        .commands
        .iter()
        .filter(|(_, options)| match &options.module {
            Some(module) => match module {
                Module::Owner => status.owner,
                Module::Utility => status.utility,
                Module::Score => status.score,
                Module::ReactionRoles => status.reaction_roles,
            },
            None => false,
        });

    // Add the commands
    let commands = guild
        .set_application_commands(&ctx.http, |commands| {
            for (name, options) in filtered {
                let command = create_command(name, options);
                commands.add_application_command(command);
            }

            commands
        })
        .await
        .expect(ERR_CMD_CREATION);

    add_permissions(ctx, &config, guild, &commands).await;
}

fn create_option(name: &str, option_config: &CommandOption) -> CreateApplicationCommandOption {
    let mut option = CreateApplicationCommandOption::default();

    option
        .kind(option_config.kind.into())
        .name(name)
        .description(&option_config.description)
        .default_option(option_config.default.unwrap_or_default())
        .required(option_config.required.unwrap_or_default())
        .set_autocomplete(option_config.autocomplete.unwrap_or_default());

    // Add options if there are any
    if let Some(choices) = &option_config.choices {
        for choice in choices {
            match choice {
                Value::Int(int) => option.add_int_choice(int, *int),
                Value::String(string) => option.add_string_choice(string, string),
            };
        }
    }

    // Add min value if it is set
    if let Some(min_value) = option_config.min_value {
        option.min_int_value(min_value);
    }

    // Add max value if it is set
    if let Some(max_value) = option_config.max_value {
        option.max_int_value(max_value);
    }

    // Add sub options if there are any
    if let Some(options) = &option_config.options {
        for (name, option_config) in options {
            let sub_option = create_option(name, option_config);
            option.add_sub_option(sub_option);
        }
    }

    option
}

/// Add permissions for a command.
pub async fn add_permissions(
    ctx: &Context,
    config: &Config,
    guild: GuildId,
    commands: &Vec<ApplicationCommand>,
) {
    // Get the partial guild to get the owner information later
    let partial_guild = guild.to_partial_guild(&ctx.http).await.expect(ERR_API_LOAD);

    // Get commands which do not have default permissions
    let commands = commands
        .iter()
        .filter(|command| !command.default_permission);

    for command in commands {
        // Get config of the command
        let command_config = config.commands.get(&command.name).expect(ERR_CMD_NOT_FOUND);

        // Get roles which should have access to the command
        let roles: Option<Vec<_>> = match command_config.permission {
            Some(permission) => Some(
                guild
                    .roles(&ctx.http)
                    .await
                    .expect(ERR_API_LOAD)
                    .iter()
                    .filter(|(_, role)| {
                        role.permissions.contains(Permissions::ADMINISTRATOR)
                            || role.permissions.contains(permission)
                    })
                    .map(|(&id, _)| u64::from(id))
                    .collect(),
            ),
            None => None,
        };

        let result = guild
            .create_application_command_permission(&ctx.http, command.id, |command_perms| {
                // Set owner execution only
                if command_config.owner.unwrap_or_default() {
                    for &owner in &config.general.owners {
                        command_perms
                            .create_permission(|perm| perm.kind(User).id(owner).permission(true));
                    }
                }

                // Set custom permissions
                if command_config.permission.is_some() {
                    // Always give permission to the guild owner
                    command_perms.create_permission(|perm| {
                        perm.kind(User)
                            .id(u64::from(partial_guild.owner_id))
                            .permission(true)
                    });

                    // TODO: Listen for guild owner change and role edit events

                    // Set custom permission for roles with the permission
                    if let Some(roles) = roles {
                        for id in roles {
                            command_perms
                                .create_permission(|perm| perm.kind(Role).id(id).permission(true));
                        }
                    };
                }

                command_perms
            })
            .await;

        if let Err(why) = result {
            warn!("{}: {:?}", ERR_CMD_SET_PERMISSION, why);
        }
    }
}

/// Parse the name of a command argument given an index.
pub fn parse_arg_name(
    args: &[ApplicationCommandInteractionDataOption],
    index: usize,
) -> Result<&str, ExecutionError> {
    let name = args
        .get(index)
        .ok_or(ExecutionError::new(ERR_CMD_ARGS_LENGTH))?
        .name
        .as_str();

    Ok(name)
}

/// Parse a command argument given an index.
pub fn parse_arg<'de, T>(
    args: &'de [ApplicationCommandInteractionDataOption],
    index: usize,
) -> Result<T, ExecutionError>
where
    T: Deserialize<'de>,
{
    let value = args
        .get(index)
        .ok_or(ExecutionError::new(ERR_CMD_ARGS_LENGTH))?
        .value
        .as_ref()
        .ok_or(ExecutionError::new(ERR_CMD_ARGS_INVALID))?;

    Deserialize::deserialize(value).map_err(|why| ExecutionError::new(&format!("{}", why)))
}

/// Parse a command argument given an index and resolve it.
pub fn parse_arg_resolved(
    args: &[ApplicationCommandInteractionDataOption],
    index: usize,
) -> Result<&ApplicationCommandInteractionDataOptionValue, ExecutionError> {
    args.get(index)
        .ok_or(ExecutionError::new(ERR_CMD_ARGS_LENGTH))?
        .resolved
        .as_ref()
        .ok_or(ExecutionError::new(ERR_CMD_ARGS_INVALID))
}
