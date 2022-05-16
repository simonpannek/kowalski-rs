#[cfg(feature = "nlp-model")]
use std::ops::Div;
use std::{str::FromStr, time::Duration};

#[cfg(feature = "nlp-model")]
use itertools::Itertools;
use linked_hash_map::LinkedHashMap;
use serde::Deserialize;
#[cfg(feature = "nlp-model")]
use serenity::model::id::{ChannelId, UserId};
use serenity::{
    builder::{
        CreateActionRow, CreateApplicationCommand, CreateApplicationCommandOption, CreateEmbed,
    },
    client::Context,
    model::{
        channel::ChannelType,
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
        prelude::application_command::ApplicationCommandInteractionDataOptionValue,
        Permissions,
    },
    utils::Colour,
};
use tracing::{error, warn};

use crate::{
    config::{Command, CommandOption, Config, Module, Value},
    database::types::ModuleStatus,
    error::KowalskiError,
    error::KowalskiError::DiscordApiError,
    strings::{
        ERR_CMD_ARGS_INVALID, ERR_CMD_CREATION, ERR_CMD_SEND_FAILURE, ERR_CMD_SET_PERMISSION,
    },
};

#[macro_export]
macro_rules! data {
    ( $ctx:expr, ( $( $type:ty ),*) ) => {
        {
            let data = $ctx.data.read().await;

            (
                $(
                data.get::<$type>().unwrap().clone(),
                )*
            )
        }
    };
    ( $ctx:expr, $type:ty ) => {
        {
            let data = $ctx.data.read().await;

            data.get::<$type>().unwrap().clone()
        }
    };
}

#[macro_export]
macro_rules! pluralize {
    ($name:expr, $variable:expr) => {
        if $variable == 1 {
            format!("{} {}", $variable, $name)
        } else {
            format!("{} {}s", $variable, $name)
        }
    };
}

pub enum InteractionResponse {
    Continue,
    Abort,
}

impl FromStr for InteractionResponse {
    type Err = KowalskiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "continue" => Ok(InteractionResponse::Continue),
            "abort" => Ok(InteractionResponse::Abort),
            _ => Err(DiscordApiError(ERR_CMD_ARGS_INVALID.to_string())),
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
) -> Result<Option<InteractionResponse>, KowalskiError> {
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
        .author_id(command.user.id.0)
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
) -> Result<(), KowalskiError> {
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
) -> Result<(), KowalskiError>
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

pub fn create_embed(title: &str, content: &str) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed.title(title).description(content);
    embed
}

async fn send_embed(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    embed: CreateEmbed,
    action_rows: Vec<CreateActionRow>,
) -> Result<(), KowalskiError> {
    command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(ChannelMessageWithSource)
                .interaction_response_data(|data| {
                    data.add_embed(embed)
                        .components(|components| components.set_action_rows(action_rows))
                })
        })
        .await?;

    Ok(())
}

async fn edit_embed(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    embed: CreateEmbed,
    action_rows: Vec<CreateActionRow>,
) -> Result<(), KowalskiError> {
    command
        .edit_original_interaction_response(&ctx.http, |response| {
            response
                .components(|components| components.set_action_rows(action_rows))
                .add_embed(embed)
        })
        .await?;

    Ok(())
}

// TODO: Avoid rate limiting here

/// Create a general command
pub fn create_command(name: &str, command_config: &Command) -> CreateApplicationCommand {
    let mut command = CreateApplicationCommand::default();

    command.name(name).description(&command_config.description);

    if let Some(permission) = command_config.permission {
        command.default_member_permissions(permission);
    }

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
                Module::Analyze => status.analyze,
            },
            None => false,
        });

    // Add the commands
    guild
        .set_application_commands(&ctx.http, |commands| {
            for (name, options) in filtered {
                let command = create_command(name, options);
                commands.add_application_command(command);
            }

            commands
        })
        .await
        .expect(ERR_CMD_CREATION);
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

    // Add channel types if there are any
    if let Some(channel_types) = &option_config.channel_types {
        let channel_types = channel_types
            .iter()
            .map(|&channel_type| channel_type.into())
            .collect::<Vec<ChannelType>>();

        option.channel_types(&channel_types);
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

/// Parse the name of a command argument given an index.
pub fn parse_arg_name(
    args: &[ApplicationCommandInteractionDataOption],
    index: usize,
) -> Result<&str, KowalskiError> {
    let name = args.get(index).unwrap().name.as_str();

    Ok(name)
}

/// Parse a command argument given an index.
pub fn parse_arg<'de, T>(
    args: &'de [ApplicationCommandInteractionDataOption],
    index: usize,
) -> Result<T, KowalskiError>
where
    T: Deserialize<'de>,
{
    let value = args.get(index).unwrap().value.as_ref().unwrap();

    let result = Deserialize::deserialize(value)?;

    Ok(result)
}

/// Parse a command argument given an index and resolve it.
pub fn parse_arg_resolved(
    args: &[ApplicationCommandInteractionDataOption],
    index: usize,
) -> Result<&ApplicationCommandInteractionDataOptionValue, KowalskiError> {
    args.get(index)
        .unwrap()
        .resolved
        .as_ref()
        .ok_or(DiscordApiError(ERR_CMD_ARGS_INVALID.to_string()))
}

#[cfg(feature = "nlp-model")]
/// Get last messages of the current channel which are relevant for analysis
pub async fn get_relevant_messages(
    ctx: &Context,
    config: &Config,
    channel_id: ChannelId,
    user_id: Option<UserId>,
) -> Result<Vec<String>, KowalskiError> {
    // Get messages to analyze
    let messages = channel_id
        .messages(&ctx.http, |builder| {
            builder.limit(config.general.nlp_max_messages)
        })
        .await?;

    let messages = messages
        .iter()
        .rev()
        .filter(|message| !message.content.is_empty())
        .filter(|message| match user_id {
            Some(user_id) => message.author.id == user_id,
            None => true,
        })
        .enumerate()
        .group_by(|(i, _)| i.div(config.general.nlp_group_size))
        .into_iter()
        .map(|(_, messages)| {
            messages
                .map(|(_, message)| {
                    format!(
                        "{}: {}",
                        message.author.name,
                        message
                            .content
                            .chars()
                            .filter(|&char| char != ':')
                            .take(config.general.nlp_max_message_length)
                            .join("")
                    )
                })
                .join("\n")
        })
        .collect();

    Ok(messages)
}
