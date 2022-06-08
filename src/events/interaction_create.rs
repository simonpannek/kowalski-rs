use serenity::{
    client::Context,
    model::interactions::{
        application_command::{ApplicationCommandInteraction, ApplicationCommandOptionType},
        autocomplete::AutocompleteInteraction,
        message_component::MessageComponentInteraction,
        Interaction, InteractionResponseType,
    },
};
use tracing::error;

use crate::{
    commands::*,
    config::{CommandType, Config},
    credits::Credits,
    data,
    error::KowalskiError,
    history::History,
    strings::{
        ERR_AUTOCOMPLETE, ERR_CMD_EXECUTION, ERR_MESSAGE_COMPONENT, ERR_USER_EXECUTION_FAILED,
        ERR_USER_TITLE,
    },
    utils::send_failure,
};

pub async fn interaction_create(ctx: &Context, interaction: Interaction) {
    match interaction {
        Interaction::ApplicationCommand(interaction) => {
            if let Err(why) = execute_command(ctx, &interaction).await {
                send_failure(ctx, &interaction, ERR_USER_TITLE, ERR_USER_EXECUTION_FAILED).await;
                error!("{}: {:?}", ERR_CMD_EXECUTION, why);
            }
        }
        Interaction::Autocomplete(interaction) => {
            if let Err(why) = answer_autocomplete(ctx, &interaction).await {
                error!("{}: {:?}", ERR_AUTOCOMPLETE, why);
            }
        }
        Interaction::MessageComponent(interaction) => {
            if let Err(why) = answer_message_component(ctx, interaction).await {
                error!("{}: {:?}", ERR_MESSAGE_COMPONENT, why);
            }
        }
        _ => {}
    }
}

async fn execute_command(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
) -> Result<(), KowalskiError> {
    // Add thinking modal
    command
        .create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
        })
        .await?;

    // Get config and credits
    let (config, credits_lock) = data!(ctx, (Config, Credits));

    // Get command name
    let name = &command.data.name;
    // Get command config
    let command_config = config.commands.get(name).unwrap();

    // Check for permissions
    let mut can_execute = true;

    // Check if the user is a owner if the comment requires it to be one
    if command_config.owner.unwrap_or_default() {
        let owners = &config.general.owners;
        if !owners.contains(&command.user.id.0) {
            can_execute = false;
        }
    }

    // Check if the user has the required permissions if there are any
    if let Some(permission) = command_config.permission {
        if let Some(member) = &command.member {
            // Get permissions of the user
            let permissions = member.permissions.unwrap();

            // Check whether the user has sufficient permissions
            can_execute = permissions.contains(permission);
        }
    }

    // Fail if user cannot execute the command
    if !can_execute {
        // TODO: Send a message
        unreachable!();
    }

    // Add command costs to user credits
    let cooldown = {
        let mut credits = credits_lock.write().await;

        credits.add_credits(&config, command.user.id.0, command_config.cost.unwrap_or(1))
    };

    // Check for cooldown
    match cooldown {
        Some(cooldown) => {
            send_failure(
                &ctx,
                &command,
                "Cooldown",
                &format!(
                    "Woah, easy there! Please wait for the cooldown to expire **({} seconds left)**.",
                    cooldown
                ),
            )
            .await;

            Ok(())
        }
        None => {
            // Execute the command
            // TODO: Use meta programming for this?
            match command_config.command_type {
                CommandType::About => about::execute(ctx, command, command_config).await,
                CommandType::Module => module::execute(ctx, command, command_config).await,
                CommandType::Modules => modules::execute(ctx, command, command_config).await,
                CommandType::Ping => ping::execute(ctx, command, command_config).await,
                CommandType::Guild => guild::execute(ctx, command, command_config).await,
                CommandType::Say => say::execute(ctx, command, command_config).await,
                CommandType::Sql => sql::execute(ctx, command, command_config).await,
                CommandType::Clear => clear::execute(ctx, command, command_config).await,
                CommandType::Reminder => reminder::execute(ctx, command, command_config).await,
                CommandType::Reminders => reminders::execute(ctx, command, command_config).await,
                CommandType::Cooldown => cooldown::execute(ctx, command, command_config).await,
                CommandType::Cooldowns => cooldowns::execute(ctx, command, command_config).await,
                CommandType::Drop => drop::execute(ctx, command, command_config).await,
                CommandType::Drops => drops::execute(ctx, command, command_config).await,
                CommandType::Emoji => emoji::execute(ctx, command, command_config).await,
                CommandType::Emojis => emojis::execute(ctx, command, command_config).await,
                CommandType::Gift => gift::execute(ctx, command, command_config).await,
                CommandType::Given => given::execute(ctx, command, command_config).await,
                CommandType::LevelUp => levelup::execute(ctx, command, command_config).await,
                CommandType::LevelUps => levelups::execute(ctx, command, command_config).await,
                CommandType::Moderation => moderation::execute(ctx, command, command_config).await,
                CommandType::Moderations => {
                    moderations::execute(ctx, command, command_config).await
                }
                CommandType::Score => score::execute(ctx, command, command_config).await,
                CommandType::Scores => scores::execute(ctx, command, command_config).await,
                CommandType::Rank => rank::execute(ctx, command, command_config).await,
                CommandType::ReactionRole => {
                    reactionrole::execute(ctx, command, command_config).await
                }
                CommandType::ReactionRoles => {
                    reactionroles::execute(ctx, command, command_config).await
                }
                #[cfg(feature = "nlp-model")]
                CommandType::Mood => mood::execute(ctx, command, command_config).await,
                #[cfg(feature = "nlp-model")]
                CommandType::Oracle => oracle::execute(ctx, command, command_config).await,
                #[cfg(feature = "nlp-model")]
                CommandType::Tldr => tldr::execute(ctx, command, command_config).await,
                #[cfg(not(feature = "nlp-model"))]
                CommandType::Mood | CommandType::Oracle | CommandType::Tldr => {
                    disabled::execute(ctx, command, command_config).await
                }
            }
        }
    }
}

async fn answer_autocomplete(
    ctx: &Context,
    autocomplete: &AutocompleteInteraction,
) -> Result<(), KowalskiError> {
    // Get read access to the history
    let (config, history_lock) = data!(ctx, (Config, History));

    // Get user, option name and the content written by the user
    let user = autocomplete.user.id;
    let (option_name, written) = {
        // Get the last option the user currently is typing
        let option = {
            let options = &autocomplete.data.options;
            let mut last = options.last().unwrap();

            while let ApplicationCommandOptionType::SubCommand = last.kind {
                last = last.options.last().unwrap();
            }

            last
        };

        let option_name = &option.name;
        let written = option.value.as_ref().unwrap();

        (option_name, written)
    };

    let choices: Vec<String> = {
        let history = history_lock.read().await;

        history
            .get_entries(user, option_name)
            .iter()
            .filter(|choice| {
                choice
                    .to_lowercase()
                    .starts_with(&written.as_str().unwrap().to_lowercase())
            })
            .cloned()
            .take(config.general.autocomplete_size)
            .collect()
    };

    autocomplete
        .create_autocomplete_response(&ctx, |response| {
            for choice in choices {
                // Choices can have a maximum length of 100 characters
                if choice.len() <= 100 {
                    response.add_string_choice(&choice, &choice);
                }
            }

            response
        })
        .await?;

    Ok(())
}

async fn answer_message_component(
    ctx: &Context,
    message_component: MessageComponentInteraction,
) -> Result<(), KowalskiError> {
    message_component
        .create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::DeferredUpdateMessage)
        })
        .await?;

    Ok(())
}
