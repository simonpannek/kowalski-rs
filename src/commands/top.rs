use std::{cmp::min, str::FromStr, time::Duration};

use serenity::{
    builder::CreateActionRow,
    client::Context,
    model::{
        channel::ReactionType,
        id::UserId,
        interactions::{
            application_command::ApplicationCommandInteraction, message_component::ButtonStyle,
        },
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
    strings::ERR_CMD_ARGS_INVALID,
    utils::{send_response, send_response_complex},
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

    // Get guild
    let guild = command.guild_id.unwrap();

    // Get top users
    let top: Vec<_> = {
        let rows = database
            .client
            .query(
                "
        SELECT user_to, COUNT(*) FILTER (WHERE upvote) upvotes,
        COUNT(*) FILTER (WHERE NOT upvote) downvotes
        FROM reactions r
        INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
        WHERE r.guild = $1::BIGINT
        GROUP BY user_to
        ORDER BY COUNT(*) FILTER (WHERE upvote) - COUNT(*) FILTER (WHERE NOT upvote) DESC, user_to
        ",
                &[&(guild.0 as i64)],
            )
            .await?;

        rows.iter()
            .map(|row| {
                let user: i64 = row.get(0);
                let upvotes: i64 = row.get(1);
                let downvotes: i64 = row.get(2);

                (UserId(user as u64), upvotes, downvotes)
            })
            .collect()
    };

    if top.is_empty() {
        send_response(
            ctx,
            command,
            command_config,
            "Top Scores",
            "Looks like there are no scores to display :(",
        )
        .await
    } else {
        let mut page_index = 0;
        let page_size = config.general.leaderboard_size;
        let page_count = (top.len() + page_size - 1) / page_size;

        // Loop through interactions until there is a timeout
        while let Some(interaction) = show_page(
            ctx,
            command,
            command_config,
            &top,
            page_index,
            page_count,
            page_size,
            &config.general.leaderboard_titles,
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
    top: &Vec<(UserId, i64, i64)>,
    index: usize,
    count: usize,
    size: usize,
    rank_titles: &Vec<String>,
    timeout: Duration,
) -> Result<Option<ComponentInteractionResponse>, KowalskiError> {
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

    // Send response
    send_response_complex(
        ctx,
        command,
        command_config,
        &format!("Top Scores (Page {}/{})", index + 1, count),
        "",
        |embed| {
            // Get start index
            let start = index * size;
            // Get page slice
            let page = {
                let end = min(start + size, top.len());
                &top[start..end]
            };

            embed.fields(
                page.iter()
                    .enumerate()
                    .map(|(i, (user, upvotes, downvotes))| {
                        let title = {
                            let index = start + i;

                            match rank_titles.get(index) {
                                Some(title) => title.clone(),
                                None => format!("#{}", index + 1),
                            }
                        };

                        (
                            title,
                            format!(
                                "{}: **{}** [+{}, -{}]",
                                user.mention(),
                                upvotes - downvotes,
                                upvotes,
                                downvotes
                            ),
                            false,
                        )
                    }),
            )
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
