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
    error::ExecutionError,
    pluralize,
    strings::{ERR_API_LOAD, ERR_DATA_ACCESS},
    utils::{parse_arg, parse_arg_resolved, send_confirmation, send_response},
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

    // Parse arguments
    let user = match parse_arg_resolved(options, 0)? {
        User(user, ..) => Ok(user),
        _ => Err(ExecutionError::new(ERR_API_LOAD)),
    }?;
    let score: i64 = parse_arg(options, 1)?;

    // Get guild and user ids
    let guild_id = i64::from(command.guild_id.ok_or(ExecutionError::new(ERR_API_LOAD))?);
    let user_id = i64::from(command.user.id);

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
                UPDATE reactions
                SET user_to = $3::BIGINT, native = false
                WHERE (guild, user_from, user_to, message, emoji) IN (
                    SELECT r.guild, user_from, user_to, message, r.emoji FROM reactions r
                    INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
                    WHERE r.guild = $1::BIGINT AND user_to = $2::BIGINT AND upvote
                    ORDER BY native
                    LIMIT $4::BIGINT
                )
                ",
                    &[&guild_id, &user_id, &i64::from(user.id), &amount],
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
