use serenity::{
    client::Context,
    model::{
        channel::{Message, Reaction, ReactionType},
        guild::Member,
        id::{ChannelId, GuildId, MessageId, RoleId, UserId},
    },
};

use crate::{
    config::Config,
    cooldowns::Cooldowns,
    data,
    database::{client::Database, types::ModuleStatus},
    error::KowalskiError,
};

pub async fn reaction_add(ctx: &Context, add_reaction: Reaction) -> Result<(), KowalskiError> {
    // Get database
    let (config, database, cooldowns_lock) = data!(ctx, (Config, Database, Cooldowns));

    // Check if the emoji is registered and get its id
    if let Some(emoji_db_id) = get_emoji_id(&add_reaction.emoji, &database).await? {
        // Get reaction data
        let (guild_id, user_from_id, user_to_id, channel_id, message_id) =
            get_reaction_data(ctx, &add_reaction).await?;

        // Get guild, channel, user_from, user_to and message ids
        let guild_db_id = database.get_guild(guild_id).await?;
        let user_from_db_id = database.get_user(guild_id, user_from_id).await?;
        let user_to_db_id = database.get_user(guild_id, user_to_id).await?;
        let channel_db_id = database.get_channel(guild_id, channel_id).await?;
        let message_db_id = database
            .get_message(guild_id, channel_id, message_id)
            .await?;

        // Self reactions do not count
        if user_from_id == user_to_id {
            return Ok(());
        }

        // Get guild status
        let status = database
            .client
            .query_opt(
                "
                SELECT status
                FROM modules
                WHERE guild = $1::BIGINT
                ",
                &[&guild_db_id],
            )
            .await?
            .map_or(ModuleStatus::default(), |row| row.get(0));

        // Get the reaction-roles to assign
        let reaction_roles: Vec<_> = if status.reaction_roles {
            let rows = database
                .client
                .query(
                    "
                    SELECT role, slots FROM reaction_roles
                    WHERE guild = $1::BIGINT AND channel = $2::BIGINT AND message = $3::BIGINT
                    AND emoji = $4::INT
                    ",
                    &[&guild_db_id, &channel_db_id, &message_db_id, &emoji_db_id],
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
        } else {
            Vec::new()
        };

        // Whether the emoji should count as a up-/downvote
        let levelup = status.score
            && reaction_roles.is_empty()
            && database
                .client
                .query_opt(
                    "
                    SELECT * FROM score_emojis
                    WHERE guild = $1::BIGINT AND emoji = $2::INT
                    ",
                    &[&guild_db_id, &emoji_db_id],
                )
                .await?
                .is_some();

        if !reaction_roles.is_empty() {
            // Get guild
            let guild_id = {
                let channel = add_reaction.channel_id.to_channel(&ctx.http).await?;
                channel.guild().map(|channel| channel.guild_id).unwrap()
            };

            // Get the member
            let mut member = guild_id.member(&ctx, user_from_id.0).await?;

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
                        UPDATE reaction_roles
                        SET slots = slots + 1
                        WHERE guild = $1::BIGINT AND channel = $2::BIGINT AND message = $3::BIGINT
                        AND emoji = $4::INT AND slots IS NOT NULL
                        ",
                            &[&guild_db_id, &channel_db_id, &message_db_id, &emoji_db_id],
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
                    UPDATE reaction_roles
                    SET slots = slots - 1
                    WHERE guild = $1::BIGINT AND channel = $2::BIGINT AND message = $3::BIGINT
                    AND emoji = $3::INT AND slots IS NOT NULL
                    ",
                                &[&guild_db_id, &channel_db_id, &message_db_id, &emoji_db_id],
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
                    .unwrap()
                    .roles
                    .iter()
                    .map(|role_id| role_id.clone())
                    .collect();

                cooldowns
                    .check_cooldown(&config, &database, guild_id, user_from_id, &roles)
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
                INSERT INTO score_reactions
                VALUES ($1::BIGINT, $2::BIGINT, $3::BIGINT, $4::BIGINT, $5::BIGINT, $6::INT,
                $7::BOOLEAN)
                ",
                        &[
                            &guild_db_id,
                            &user_from_db_id,
                            &user_to_db_id,
                            &channel_db_id,
                            &message_db_id,
                            &emoji_db_id,
                            &true,
                        ],
                    )
                    .await?;

                // Get guild
                let guild = {
                    let channel = add_reaction.channel_id.to_channel(&ctx.http).await?;
                    channel.guild().map(|channel| channel.guild_id).unwrap()
                };

                // Update the roles of the user
                let mut member = guild.member(&ctx, user_to_id.0).await?;
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
) -> Result<(), KowalskiError> {
    // Get database
    let database = data!(ctx, Database);

    // Check if the emoji is registered
    if let Some(emoji_db_id) = get_emoji_id(&removed_reaction.emoji, &database).await? {
        // Get reaction data
        let (guild_id, user_from_id, _, channel_id, message_id) =
            get_reaction_data(ctx, &removed_reaction).await?;

        // Get guild, channel, user_from and message ids
        let guild_db_id = database.get_guild(guild_id).await?;
        let user_from_db_id = database.get_user(guild_id, user_from_id).await?;
        let channel_db_id = database.get_channel(guild_id, channel_id).await?;
        let message_db_id = database
            .get_message(guild_id, channel_id, message_id)
            .await?;

        // Get user of which the reaction was removed
        let user_to_db_id = {
            let row = database
                .client
                .query_opt(
                    "
            SELECT user_to FROM score_reactions
            WHERE guild = $1::BIGINT AND user_from = $2::BIGINT AND channel = $3::BIGINT
            AND message = $4::BIGINT AND emoji = $5::INT
            ",
                    &[
                        &guild_db_id,
                        &user_from_db_id,
                        &channel_db_id,
                        &message_db_id,
                        &emoji_db_id,
                    ],
                )
                .await?;

            row.map(|row| row.get::<_, i64>(0))
        };

        if let Some(user_to_db_id) = user_to_db_id {
            // Delete possible reaction emoji
            database
                .client
                .execute(
                    "
        DELETE FROM score_reactions
        WHERE guild = $1::BIGINT AND user_from = $2::BIGINT AND channel = $3::BIGINT
        AND message = $4::BIGINT AND emoji = $5::INT
        ",
                    &[
                        &guild_db_id,
                        &user_from_db_id,
                        &channel_db_id,
                        &message_db_id,
                        &emoji_db_id,
                    ],
                )
                .await?;

            // Get guild
            let guild = {
                let channel = removed_reaction.channel_id.to_channel(&ctx.http).await?;
                channel.guild().map(|channel| channel.guild_id).unwrap()
            };

            // Update the roles of the user
            let mut member = guild.member(&ctx, user_to_db_id as u64).await?;
            update_roles(&ctx, &database, &mut member).await?;

            // Auto moderate the message if necessary
            let message = removed_reaction.message(&ctx.http).await?;
            auto_moderate(&ctx, &database, guild, message).await?;
        }
    }

    Ok(())
}

pub async fn reaction_remove_all(
    ctx: Context,
    channel_id: ChannelId,
    removed_from_message_id: MessageId,
) -> Result<(), KowalskiError> {
    // Get database
    let database = data!(ctx, Database);

    let guild_id = {
        let channel = channel_id.to_channel(&ctx.http).await?;
        channel.guild().map(|channel| channel.guild_id)
    };

    // Check if there is a guild
    if let Some(guild_id) = guild_id {
        // Get guild id
        let guild_db_id = database.get_guild(guild_id).await?;
        let channel_db_id = database.get_channel(guild_id, channel_id).await?;
        let message_db_id = database
            .get_message(guild_id, channel_id, removed_from_message_id)
            .await?;

        // Delete possible reaction emojis
        database
            .client
            .execute(
                "
        DELETE FROM score_reactions
        WHERE guild = $1::BIGINT AND channel = $2::BIGINT AND message = $3::BIGINT
        ",
                &[&guild_db_id, &channel_db_id, &message_db_id],
            )
            .await?;

        // Update the roles of the user
        let message = channel_id
            .message(&ctx.http, removed_from_message_id)
            .await?;
        let mut member = guild_id.member(&ctx, message.author.id).await?;
        update_roles(&ctx, &database, &mut member).await?;

        let message = channel_id
            .message(&ctx.http, removed_from_message_id)
            .await?;
        auto_moderate(&ctx, &database, guild_id, message).await?;
    }

    Ok(())
}

async fn get_emoji_id(
    emoji: &ReactionType,
    database: &Database,
) -> Result<Option<i32>, KowalskiError> {
    let rows = match emoji {
        ReactionType::Unicode(string) => {
            database
                .client
                .query(
                    "
                SELECT id FROM emojis
                WHERE unicode = $1::TEXT
                ",
                    &[string],
                )
                .await?
        }
        ReactionType::Custom { id: emoji_id, .. } => {
            database
                .client
                .query(
                    "
                    SELECT id FROM emojis
                    WHERE guild_emoji = $1::BIGINT
                    ",
                    &[&(emoji_id.0 as i64)],
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
) -> Result<(GuildId, UserId, UserId, ChannelId, MessageId), KowalskiError> {
    let guild_id = reaction.guild_id.unwrap();
    let user_from_id = reaction.user_id.unwrap();
    let user_to_id = {
        let message = reaction.message(&ctx.http).await?;
        message.author.id
    };
    let channel_id = reaction.channel_id;
    let message_id = reaction.message_id;

    Ok((guild_id, user_from_id, user_to_id, channel_id, message_id))
}

async fn update_roles(
    ctx: &Context,
    database: &Database,
    member: &mut Member,
) -> Result<(), KowalskiError> {
    // Never update roles of bots
    if member.user.bot {
        return Ok(());
    }

    // Get guild and user ids
    let guild_db_id = database.get_guild(member.guild_id).await?;
    let user_db_id = database.get_user(member.guild_id, member.user.id).await?;

    // Get the score of the user
    let score = {
        let row = database
            .client
            .query_one(
                "
        SELECT SUM(CASE WHEN upvote THEN 1 ELSE -1 END) score
        FROM score_reactions r
        INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
        WHERE r.guild = $1::BIGINT AND user_to = $2::BIGINT
        ",
                &[&guild_db_id, &user_db_id],
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
                &[&guild_db_id],
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
            WITH role_score AS (
                SELECT score FROM score_roles
                WHERE guild = $1::BIGINT AND score <= $2::BIGINT
                ORDER BY score DESC
                LIMIT 1
            )

            SELECT role FROM score_roles
            WHERE guild = $1::BIGINT
            AND score = (SELECT score FROM role_score)
            ",
                &[&guild_db_id, &score],
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
    guild_id: GuildId,
    message: Message,
) -> Result<(), KowalskiError> {
    // Get guild and message ids
    let guild_db_id = database.get_guild(guild_id).await?;
    let message_db_id = database
        .get_message(guild_id, message.channel_id, message.id)
        .await?;

    // Get scores of auto-pin and auto-delete
    let pin_score = {
        let row = database
            .client
            .query_opt(
                "
        SELECT score FROM score_auto_pin
        WHERE guild = $1::BIGINT
        ",
                &[&guild_db_id],
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
                &[&guild_db_id],
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
                SELECT SUM(CASE WHEN upvote THEN 1 ELSE -1 END) FROM score_reactions r
                INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
                WHERE r.guild = $1::BIGINT AND message = $2::BIGINT
                ",
                    &[&guild_db_id, &message_db_id],
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
