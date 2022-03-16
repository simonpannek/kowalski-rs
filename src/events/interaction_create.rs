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
    error::ExecutionError,
    history::History,
    strings::{
        ERR_API_LOAD, ERR_AUTOCOMPLETE, ERR_CMD_EXECUTION, ERR_CMD_NOT_FOUND, ERR_CMD_PERMISSION,
        ERR_DATA_ACCESS, ERR_MESSAGE_COMPONENT, ERR_USER_EXECUTION_FAILED, ERR_USER_TITLE,
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
) -> Result<(), ExecutionError> {
    // Add thinking modal
    command
        .create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
        })
        .await?;

    // Get config and credits
    let (config, credits_lock) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().expect(ERR_DATA_ACCESS).clone();
        let credits_lock = data.get::<Credits>().expect(ERR_DATA_ACCESS).clone();

        (config, credits_lock)
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

    // Add command costs to user credits
    let _cooldown = {
        let mut credits = credits_lock.write().await;

        credits.add_credits(&config, command.user.id.0, command_config.cost.unwrap_or(1))
    };

    // Execute the command
    match command_config.command_type {
        CommandType::About => about::execute(ctx, command, command_config).await,
        CommandType::Info => info::execute(ctx, command, command_config).await,
        CommandType::Module => module::execute(ctx, command, command_config).await,
        CommandType::Ping => ping::execute(ctx, command, command_config).await,
        CommandType::Guild => guilds::execute(ctx, command, command_config).await,
        CommandType::Say => say::execute(ctx, command, command_config).await,
        CommandType::Sql => sql::execute(ctx, command, command_config).await,
        CommandType::Clear => clear::execute(ctx, command, command_config).await,
        CommandType::Moderate => moderate::execute(ctx, command, command_config).await,
        CommandType::Cooldown => cooldown::execute(ctx, command, command_config).await,
        CommandType::Emoji => emoji::execute(ctx, command, command_config).await,
        CommandType::Given => given::execute(ctx, command, command_config).await,
        CommandType::LevelUp => levelup::execute(ctx, command, command_config).await,
        CommandType::Score => score::execute(ctx, command, command_config).await,
        CommandType::Rank => rank::execute(ctx, command, command_config).await,
        CommandType::Top => top::execute(ctx, command, command_config).await,
        CommandType::ReactionRole => reactionrole::execute(ctx, command, command_config).await,
    }
}

async fn answer_autocomplete(
    ctx: &Context,
    autocomplete: &AutocompleteInteraction,
) -> Result<(), ExecutionError> {
    // Get read access to the history
    let (config, history_lock) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().expect(ERR_DATA_ACCESS).clone();
        let history_lock = data.get::<History>().expect(ERR_DATA_ACCESS).clone();

        (config, history_lock)
    };

    // Get user, option name and the content written by the user
    let user = autocomplete.user.id;
    let (option_name, written) = {
        // Get the last option the user currently is typing
        let option = {
            let options = &autocomplete.data.options;
            let mut last = options.last().ok_or(ExecutionError::new(ERR_API_LOAD))?;

            while let ApplicationCommandOptionType::SubCommand = last.kind {
                last = last
                    .options
                    .last()
                    .ok_or(ExecutionError::new(ERR_API_LOAD))?;
            }

            last
        };

        let option_name = &option.name;
        let written = option
            .value
            .as_ref()
            .ok_or(ExecutionError::new(ERR_API_LOAD))?;

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
                response.add_string_choice(&choice, &choice);
            }

            response
        })
        .await
        .map_err(|why| ExecutionError::new(&format!("{}", why)))
}

async fn answer_message_component(
    ctx: &Context,
    message_component: MessageComponentInteraction,
) -> Result<(), ExecutionError> {
    message_component
        .create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::DeferredUpdateMessage)
        })
        .await
        .map_err(|why| ExecutionError::new(&format!("{}", why)))
}
