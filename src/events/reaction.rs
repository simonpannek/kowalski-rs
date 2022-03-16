use serenity::{
    client::Context,
    model::{
        channel::{Reaction, ReactionType, Message},
        guild::Member,
        id::{ChannelId, MessageId, RoleId, GuildId},
    },
};

use crate::{
    config::Config,
    cooldowns::Cooldowns,
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

        // Self reactions do not count
        if user_from == user_to {
            return Ok(());
        }

        // Get the reaction-roles to assign
        let reaction_roles: Vec<_> = {
            let rows = database
                .client
                .query(
                    "
                    SELECT role, slots FROM reaction_roles
                    WHERE guild = $1::BIGINT AND message = $2::BIGINT AND emoji = $3::INT
                    ",
                    &[&guild, &message, &emoji],
                )
                .await?;

            rows.iter()
                .map(|row| {
                    (
                        RoleId(row.get::<_, i64>(0) as u64),
                        row.get::<_, Option<i32>>(1),
                    )
                })
                .collect()
        };
        // Whether the emoji should count as a up-/downvote
        let levelup = reaction_roles.is_empty()
            && database
                .client
                .query_opt(
                    "
                    SELECT * FROM score_emojis
                    WHERE guild = $1::BIGINT AND emoji = $2::INT
                    ",
                    &[&guild, &emoji],
                )
                .await?
                .is_some();

        if !reaction_roles.is_empty() {
            // Get guild
            let guild_id = {
                let channel = add_reaction.channel_id.to_channel(&ctx.http).await?;
                channel
                    .guild()
                    .map(|channel| channel.guild_id)
                    .ok_or(ExecutionError::new(ERR_API_LOAD))?
            };
            // Get the member
            let mut member = guild_id.member(&ctx, user_from as u64).await?;

            // Never give roles to bots
            if member.user.bot {
                return Ok(());
            }

            // Remove the reaction
            add_reaction.delete(&ctx.http).await?;

            for (role, slots) in reaction_roles {
                if member.roles.contains(&role) {
                    // Increment slots
                    database
                        .client
                        .execute(
                            "
                        UPDATE reaction_roles SET slots = slots + 1
                        WHERE guild = $1::BIGINT AND message = $2::BIGINT
                            AND emoji = $3::INT AND slots IS NOT NULL
                        ",
                            &[&guild, &message, &emoji],
                        )
                        .await?;

                    // Remove role from user
                    member.remove_role(&ctx.http, role).await?;
                } else {
                    if !matches!(slots, Some(0)) {
                        // Decrement slots
                        database
                            .client
                            .execute(
                                "
                    UPDATE reaction_roles SET slots = slots - 1
                    WHERE guild = $1::BIGINT AND message = $2::BIGINT
                        AND emoji = $3::INT AND slots IS NOT NULL
                    ",
                                &[&guild, &message, &emoji],
                            )
                            .await?;

                        // Add role to user
                        member.add_role(&ctx.http, role).await?;
                    }
                }
            }
        } else if levelup {
            // Check for cooldown
            let cooldown_active = {
                let mut cooldowns = cooldowns_lock.write().await;

                // Get role ids of user
                let roles: Vec<_> = add_reaction
                    .member
                    .as_ref()
                    .ok_or(ExecutionError::new(ERR_API_LOAD))?
                    .roles
                    .iter()
                    .map(|&role| u64::from(role))
                    .collect();

                cooldowns
                    .check_cooldown(&config, &database, guild as u64, user_from as u64, &roles)
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

                // Get guild
                let guild = {
                    let channel = add_reaction.channel_id.to_channel(&ctx.http).await?;
                    channel
                        .guild()
                        .map(|channel| channel.guild_id)
                        .ok_or(ExecutionError::new(ERR_API_LOAD))?
                };

                // Update the roles of the user
                let mut member = guild.member(&ctx, user_to as u64).await?;
                update_roles(&ctx, &database, &mut member).await?;

                // Auto moderate the message if necessary
                let message = add_reaction.message(&ctx.http).await?;
                auto_moderate(&ctx, &database, guild, message).await?;
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

        // Get guild
        let guild = {
            let channel = removed_reaction.channel_id.to_channel(&ctx.http).await?;
            channel
                .guild()
                .map(|channel| channel.guild_id)
                .ok_or(ExecutionError::new(ERR_API_LOAD))?
        };

        // Update the roles of the user
        let mut member = guild.member(&ctx, user_to as u64).await?;
        update_roles(&ctx, &database, &mut member).await?;

        // Auto moderate the message if necessary
        let message = removed_reaction.message(&ctx.http).await?;
        auto_moderate(&ctx, &database, guild, message).await?;
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

        // Update the roles of the user
        let message = channel_id
            .message(&ctx.http, removed_from_message_id)
            .await?;
        let mut member = guild.member(&ctx, message.author.id).await?;
        update_roles(&ctx, &database, &mut member).await?;

        let message = channel_id
            .message(&ctx.http, removed_from_message_id)
            .await?;
        auto_moderate(&ctx, &database, guild, message).await?;
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

async fn update_roles(
    ctx: &Context,
    database: &Database,
    member: &mut Member,
) -> Result<(), ExecutionError> {
    // Never update roles of bots
    if member.user.bot {
        return Ok(());
    }

    // Get the score of the user
    let score = {
        let row = database
            .client
            .query_one(
                "
        SELECT SUM(CASE WHEN upvote THEN 1 ELSE -1 END) score
        FROM reactions r
        INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
        WHERE r.guild = $1::BIGINT AND user_to = $2::BIGINT
        ",
                &[&i64::from(member.guild_id), &i64::from(member.user.id)],
            )
            .await?;

        row.get::<_, Option<i64>>(0).unwrap_or_default()
    };

    // Get all roles handled by the level-up system
    let handled: Vec<_> = {
        let rows = database
            .client
            .query(
                "SELECT DISTINCT role FROM score_roles WHERE guild = $1::BIGINT",
                &[&i64::from(member.guild_id)],
            )
            .await?;

        rows.iter()
            .map(|row| RoleId(row.get::<_, i64>(0) as u64))
            .collect()
    };
    // Get all roles the user should currently have
    let current: Vec<_> = {
        let rows = database
            .client
            .query(
                "
            SELECT role FROM score_roles
            WHERE guild = $1::BIGINT AND score = (
                SELECT score FROM score_roles
                WHERE guild = $1::BIGINT AND score <= $2::BIGINT
                ORDER BY score DESC
                LIMIT 1
            )
            ",
                &[&i64::from(member.guild_id), &score],
            )
            .await?;

        rows.iter()
            .map(|row| RoleId(row.get::<_, i64>(0) as u64))
            .collect()
    };

    // Current roles of the user
    let roles = &member.roles;

    // Filter roles the user should have but doesn't
    let add: Vec<_> = current
        .iter()
        .filter(|role| !roles.contains(role))
        .copied()
        .collect();
    // Filter roles the user shouldn't have but does
    let remove: Vec<_> = roles
        .iter()
        .filter(|role| handled.contains(role) && !current.contains(role))
        .copied()
        .collect();

    // Add new roles
    if !add.is_empty() {
        member.add_roles(&ctx.http, &add[..]).await?;
    }
    // Remove old roles
    if !remove.is_empty() {
        member.remove_roles(&ctx.http, &remove[..]).await?;
    }

    Ok(())
}

async fn auto_moderate(
    ctx: &Context,
    database: &Database,
    guild: GuildId,
    message: Message,
) -> Result<(), ExecutionError> {
    // Get scores of auto-pin and auto-delete
    let pin_score = {
        let row = database
            .client
            .query_opt(
                "
        SELECT score FROM score_auto_pin
        WHERE guild = $1::BIGINT
        ",
                &[&i64::from(guild)],
            )
            .await?;

        row.map(|row| row.get::<_, i64>(0))
    };
    let delete_score = {
        let row = database
            .client
            .query_opt(
                "
        SELECT score FROM score_auto_delete
        WHERE guild = $1::BIGINT
        ",
                &[&i64::from(guild)],
            )
            .await?;

        row.map(|row| row.get::<_, i64>(0))
    };

    // Check whether auto moderation is enabled
    if pin_score.is_some() || delete_score.is_some() {
        // Get score of the message
        let score = {
            let row = database
                .client
                .query_one(
                    "
                SELECT SUM(CASE WHEN upvote THEN 1 ELSE -1 END) FROM reactions r
                INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
                WHERE r.guild = $1::BIGINT AND message = $2::BIGINT
                ",
                    &[&i64::from(guild), &i64::from(message.id)],
                )
                .await?;

            row.get::<_, Option<i64>>(0).unwrap_or_default()
        };

        // Check whether message should get pinned
        if !message.pinned {
            if let Some(pin_score) = pin_score {
                // Check whether scores share the same sign
                if (score >= 0) == (pin_score >= 0) {
                    if score.abs() >= pin_score.abs() {
                        // Pin the message
                        message.pin(&ctx.http).await?;
                    }
                }
            }
        }

        // Check whether message should get deleted
        if let Some(delete_score) = delete_score {
            // Check whether scores share the same sign
            if (score >= 0) == (delete_score >= 0) {
                if score.abs() >= delete_score.abs() {
                    // Delete the message
                    message.delete(&ctx.http).await?;
                }
            }
        }
    }

    Ok(())
}
