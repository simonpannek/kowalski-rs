use serenity::{
    client::Context,
    model::{
        channel::{Reaction, ReactionType},
        id::{ChannelId, MessageId},
    },
};

use crate::cooldowns::Cooldowns;
use crate::{
    config::Config,
    database::client::Database,
    error::ExecutionError,
    strings::{ERR_API_LOAD, ERR_DATA_ACCESS},
};

pub async fn reaction_add(ctx: &Context, add_reaction: Reaction) -> Result<(), ExecutionError> {
    // Get database
    let (config, database, cooldowns_lock) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().expect(ERR_DATA_ACCESS).clone();
        let database = data.get::<Database>().expect(ERR_DATA_ACCESS).clone();
        let cooldowns_lock = data.get::<Cooldowns>().expect(ERR_DATA_ACCESS).clone();

        (config, database, cooldowns_lock)
    };

    // Check if the emoji is registered
    if let Some(emoji) = get_emoji_id(&add_reaction.emoji, &database).await? {
        // Get reaction data
        let (guild, user_from, user_to, message) = get_reaction_data(ctx, &add_reaction).await?;
        // Check whether the emoji is a reaction emoji counted as a up-/downvote
        if database
            .client
            .query_opt(
                "
        SELECT * FROM reaction_emojis
        WHERE guild = $1::BIGINT AND emoji = $2::INT
        ",
                &[&guild, &emoji],
            )
            .await?
            .is_some()
        {
            // Self reactions do not count
            if user_from == user_to {
                return Ok(());
            }

            // Check for cooldown
            let cooldown_active = {
                let mut cooldowns = cooldowns_lock.write().await;

                cooldowns
                    .check_cooldown(&config, &database, guild as u64, user_from as u64)
                    .await?
            };

            if cooldown_active {
                // Remove reaction
                add_reaction.delete(&ctx.http).await?;
            } else {
                // Insert row
                database
                    .client
                    .execute(
                        "
                INSERT INTO reactions
                VALUES ($1::BIGINT, $2::BIGINT, $3::BIGINT, $4::BIGINT, $5::INT, $6::BOOLEAN)
                ",
                        &[&guild, &user_from, &user_to, &message, &emoji, &true],
                    )
                    .await?;
            }
        }
    }

    Ok(())
}

pub async fn reaction_remove(
    ctx: &Context,
    removed_reaction: Reaction,
) -> Result<(), ExecutionError> {
    // Get database
    let database = {
        let data = ctx.data.read().await;

        data.get::<Database>().expect(ERR_DATA_ACCESS).clone()
    };

    // Check if the emoji is registered
    if let Some(emoji) = get_emoji_id(&removed_reaction.emoji, &database).await? {
        // Get reaction data
        let (guild, user_from, user_to, message) =
            get_reaction_data(ctx, &removed_reaction).await?;

        // Delete possible reaction emoji
        database
            .client
            .execute(
                "
        DELETE FROM reactions
        WHERE guild = $1::BIGINT AND user_from = $2::BIGINT AND user_to = $3::BIGINT
            AND message = $4::BIGINT AND emoji = $5::INT
        ",
                &[&guild, &user_from, &user_to, &message, &emoji],
            )
            .await?;
    }

    Ok(())
}

pub async fn reaction_remove_all(
    ctx: Context,
    channel_id: ChannelId,
    removed_from_message_id: MessageId,
) -> Result<(), ExecutionError> {
    // Get database
    let database = {
        let data = ctx.data.read().await;

        data.get::<Database>().expect(ERR_DATA_ACCESS).clone()
    };

    let guild = {
        let channel = channel_id.to_channel(&ctx.http).await?;
        channel.guild().map(|channel| channel.guild_id)
    };

    // Check if there is a guild
    if let Some(guild) = guild {
        // Delete possible reaction emojis
        database
            .client
            .execute(
                "
        DELETE FROM reactions
        WHERE guild = $1::BIGINT AND message = $2::INT
        ",
                &[&i64::from(guild), &i64::from(removed_from_message_id)],
            )
            .await?;
    }

    Ok(())
}

async fn get_emoji_id(
    emoji: &ReactionType,
    database: &Database,
) -> Result<Option<i32>, ExecutionError> {
    let rows = match emoji {
        ReactionType::Unicode(string) => {
            database
                .client
                .query("SELECT id FROM emojis WHERE unicode = $1::TEXT", &[string])
                .await?
        }
        ReactionType::Custom { id, .. } => {
            database
                .client
                .query(
                    "SELECT id FROM emojis WHERE emoji_guild = $1::BIGINT",
                    &[&i64::from(id.clone())],
                )
                .await?
        }
        _ => unreachable!(),
    };

    Ok(rows.first().map(|row| row.get(0)))
}

async fn get_reaction_data(
    ctx: &Context,
    reaction: &Reaction,
) -> Result<(i64, i64, i64, i64), ExecutionError> {
    let guild = i64::from(reaction.guild_id.ok_or(ExecutionError::new(ERR_API_LOAD))?);
    let user_from = i64::from(reaction.user_id.ok_or(ExecutionError::new(ERR_API_LOAD))?);
    let user_to = {
        let message = reaction.message(&ctx.http).await?;
        i64::from(message.author.id)
    };
    let message = i64::from(reaction.message_id);

    Ok((guild, user_from, user_to, message))
}
