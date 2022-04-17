use std::{cmp::min, time::Duration};

use serenity::{
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue::User,
    },
    prelude::Mentionable,
};

use crate::utils::InteractionResponse;
use crate::{
    config::{Command, Config},
    database::client::Database,
    error::KowalskiError,
    pluralize,
    utils::{parse_arg, parse_arg_resolved, send_confirmation, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get config and database
    let (config, database) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().unwrap().clone();
        let database = data.get::<Database>().unwrap().clone();

        (config, database)
    };

    let options = &command.data.options;

    // Parse arguments
    let user = match parse_arg_resolved(options, 0)? {
        User(user, ..) => user,
        _ => unreachable!(),
    };
    let score: i64 = parse_arg(options, 1)?;

    // Get guild and user ids
    let guild_id = command.guild_id.unwrap().0 as i64;
    let user_id = command.user.id.0 as i64;

    // Calculate amount to gift
    let amount = {
        // Select all upvotes the user has received
        let row = database
            .client
            .query_one(
                "
        SELECT COUNT(*) FROM reactions r
        INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
        WHERE r.guild = $1::BIGINT AND user_to = $2::BIGINT AND upvote
        ",
                &[&guild_id, &user_id],
            )
            .await?;

        let upvotes = row.get::<_, Option<_>>(0).unwrap_or_default();

        min(score, upvotes)
    };

    let title = format!(
        "Gifting {} {} to {}",
        amount,
        pluralize!("reaction", amount),
        user.name
    );

    // Prevent user from gifting to themselves
    if user.id == command.user.id {
        return send_response(
            ctx,
            command,
            command_config,
            &title,
            "You can't give reactions to yourself...",
        )
        .await;
    }

    // Check for the interaction response
    let response = send_confirmation(
        ctx,
        command,
        command_config,
        &format!(
            "Are you really sure you want to give {} reactions to {}?
                This cannot be reversed!",
            amount,
            user.mention()
        ),
        Duration::from_secs(config.general.interaction_timeout),
    )
    .await?;

    match response {
        Some(InteractionResponse::Continue) => {
            // Move reactions to the new user
            let altered_rows = database
                .client
                .execute(
                    "
                WITH to_update AS (
                    SELECT r.guild, user_from, user_to, channel, message, r.emoji
                    FROM reactions r
                    INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
                    WHERE r.guild = $1::BIGINT AND user_to = $2::BIGINT AND upvote
                    ORDER BY native
                    LIMIT $4::BIGINT
                )

                UPDATE reactions
                SET user_to = $3::BIGINT, native = false
                WHERE (guild, user_from, user_to, channel, message, emoji) IN to_update
                ",
                    &[&guild_id, &user_id, &(user.id.0 as i64), &amount],
                )
                .await?;

            send_response(
                ctx,
                command,
                command_config,
                &title,
                &format!(
                    "Successfully gifted {} reactions to {}.",
                    altered_rows,
                    user.mention()
                ),
            )
            .await
        }
        Some(InteractionResponse::Abort) => {
            send_response(ctx, command, command_config, &title, "Aborted the action.").await
        }
        None => Ok(()),
    }
}
