use std::str::FromStr;
use std::{cmp::min, time::Duration};

use chrono::{DateTime, Utc};
use serenity::{
    builder::CreateActionRow,
    client::Context,
    model::{
        channel::ReactionType,
        id::{ChannelId, UserId},
        interactions::{
            application_command::{
                ApplicationCommandInteraction,
                ApplicationCommandInteractionDataOptionValue as DataOptionValue,
            },
            message_component::ButtonStyle,
        },
        user::User,
    },
    prelude::Mentionable,
};

use crate::{
    config::Command,
    config::Config,
    data,
    database::client::Database,
    error::KowalskiError,
    error::KowalskiError::DiscordApiError,
    row_id,
    strings::ERR_CMD_ARGS_INVALID,
    utils::{parse_arg_resolved, send_response, send_response_complex},
};

enum ComponentInteractionResponse {
    Left,
    Right,
}

impl FromStr for ComponentInteractionResponse {
    type Err = KowalskiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "left" => Ok(ComponentInteractionResponse::Left),
            "right" => Ok(ComponentInteractionResponse::Right),
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

    let guild_id = command.guild_id.unwrap();

    // Get guild id
    let guild_db_id = database.get_guild(guild_id).await?;

    let options = &command.data.options;

    let user = if !options.is_empty() {
        let user = match parse_arg_resolved(options, 0)? {
            DataOptionValue::User(user, ..) => user,
            _ => unreachable!(),
        };

        Some(user)
    } else {
        None
    };

    // Get reminders depending on the given argument
    let reminders: Vec<_> = match user {
        Some(user) => {
            // Get user id
            let user_db_id = database.get_user(guild_id, user.id).await?;

            let rows = database
                .client
                .query(
                    "
            SELECT channel, time, content
            FROM reminders
            WHERE guild = $1::BIGINT AND \"user\" = $2::BIGINT
            ORDER BY time
            ",
                    &[&guild_db_id, &user_db_id],
                )
                .await?;

            rows.iter()
                .map(|row| {
                    (
                        row_id!(ChannelId, row, 0),
                        None,
                        row.get::<_, DateTime<Utc>>(1),
                        row.get(2),
                    )
                })
                .collect()
        }
        None => {
            // Query the next reminder_list_size reminders
            let rows = database
                .client
                .query(
                    "
            SELECT channel, \"user\", time, content
            FROM reminders
            WHERE guild = $1::BIGINT
            ORDER BY time
            ",
                    &[&guild_db_id],
                )
                .await?;

            rows.iter()
                .map(|row| {
                    (
                        row_id!(ChannelId, row, 0),
                        Some(row_id!(UserId, row, 1)),
                        row.get::<_, DateTime<Utc>>(2),
                        row.get(3),
                    )
                })
                .collect()
        }
    };

    if reminders.is_empty() {
        let title = match user {
            Some(user) => format!("Reminders of {}", user.name),
            None => "Reminders".to_string(),
        };

        send_response(
            ctx,
            command,
            command_config,
            &title,
            "Looks like there are no reminders to display :(",
        )
        .await
    } else {
        let mut page_index = 0;
        let page_size = config.general.reminder_list_size;
        let page_count = (reminders.len() + page_size - 1) / page_size;

        // Loop through interactions until there is a timeout
        while let Some(interaction) = show_page(
            ctx,
            command,
            command_config,
            user,
            &reminders,
            page_index,
            page_count,
            page_size,
            Duration::from_secs(config.general.interaction_timeout),
        )
        .await?
        {
            match interaction {
                ComponentInteractionResponse::Left => page_index -= 1,
                ComponentInteractionResponse::Right => page_index += 1,
            }
        }

        // Remove components
        command
            .edit_original_interaction_response(&ctx.http, |response| {
                response.components(|components| components)
            })
            .await?;

        Ok(())
    }
}

async fn show_page(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
    user: Option<&User>,
    reminders: &Vec<(ChannelId, Option<UserId>, DateTime<Utc>, String)>,
    index: usize,
    count: usize,
    size: usize,
    timeout: Duration,
) -> Result<Option<ComponentInteractionResponse>, KowalskiError> {
    // Get config
    let config = data!(ctx, Config);

    let mut row = CreateActionRow::default();
    row.create_button(|button| {
        button
            .emoji(ReactionType::Unicode("⬅️".to_string()))
            .custom_id("left")
            .style(ButtonStyle::Secondary)
            .disabled(index == 0)
    })
    .create_button(|button| {
        button
            .emoji(ReactionType::Unicode("➡️".to_string()))
            .custom_id("right")
            .style(ButtonStyle::Secondary)
            .disabled(index >= count - 1)
    });

    let title = match user {
        Some(user) => format!("Reminders of {} (Page {}/{})", user.name, index + 1, count),
        None => format!("Reminders (Page {}/{})", index + 1, count),
    };

    // Send response
    send_response_complex(
        ctx,
        command,
        command_config,
        &title,
        "",
        |embed| {
            // Get start index
            let start = index * size;
            // Get page slice
            let page = {
                let end = min(start + size, reminders.len());
                &reminders[start..end]
            };

            embed.fields(page.iter().map(|(channel_id, user_id, datetime, content)| {
                // Cut of content after a certain length
                let content = &content[..min(
                    config.general.reminder_list_max_message_length,
                    content.len(),
                )];

                (
                    format!("<t:{}:f>", datetime.timestamp()),
                    if let Some(user_id) = user_id {
                        format!(
                            "Reminder of {} in {}: {}",
                            user_id.clone().mention().to_string(),
                            channel_id.clone().mention(),
                            content
                        )
                    } else {
                        format!("Reminder in {}: {}", channel_id.clone().mention(), content)
                    },
                    false,
                )
            }))
        },
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
        Some(interaction) => Some(ComponentInteractionResponse::from_str(
            interaction.data.custom_id.as_str(),
        )?),
        None => None,
    };

    Ok(response)
}
