use std::time::Duration;

use serenity::{
    client::Context,
    model::{
        id::{ChannelId, GuildId, MessageId, RoleId, UserId},
        interactions::application_command::ApplicationCommandInteraction,
    },
};

use crate::{
    config::{Command, Config},
    data,
    database::client::Database,
    error::KowalskiError,
    utils::{send_confirmation, send_response, InteractionResponse},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get config and database
    let (config, _database) = data!(ctx, (Config, Database));

    let title = "Clean database tables";

    // Check for the interaction response
    let response = send_confirmation(
        ctx,
        command,
        command_config,
        "Are you really sure you want to clean the database tables?
            This cannot be reversed!",
        Duration::from_secs(config.general.interaction_timeout),
    )
    .await?;

    match response {
        Some(InteractionResponse::Continue) => {
            // Clean all the database tables
            clean_database(ctx).await;

            send_response(
                &ctx,
                &command,
                command_config,
                &title,
                "I successfully cleaned all tables.
                Please make sure no data was lost.",
            )
            .await
        }
        Some(InteractionResponse::Abort) => {
            send_response(ctx, command, command_config, title, "Aborted the action.").await
        }
        None => Ok(()),
    }
}

macro_rules! get_guild_ids {
    ($database:expr, $guild_id:expr, $table_name:expr, $parameter_name:expr, $object:expr) => {{
        let rows = $database
            .client
            .query(
                &format!(
                    "SELECT {} FROM {} WHERE guild = $1",
                    $parameter_name, $table_name
                ),
                &[&($guild_id.0 as i64)],
            )
            .await
            .unwrap();

        rows.iter()
            .map(|row| $object(row.get::<_, i64>(0) as u64))
            .collect()
    }};
}

async fn clean_database(ctx: &Context) {
    // Get database
    let database = data!(ctx, Database);

    // Get all guild ids currently tracked
    let guild_ids: Vec<_> = {
        let rows = database
            .client
            .query("SELECT guild FROM guilds", &[])
            .await
            .unwrap();

        rows.iter()
            .map(|row| GuildId(row.get::<_, i64>(0) as u64))
            .collect()
    };

    for guild_id in guild_ids {
        match guild_id.to_partial_guild(&ctx.http).await {
            // Bot is still on the guild
            Ok(partial_guild) => {
                // Get channels and roles of the guild
                let channels = partial_guild.channels(&ctx.http).await.unwrap();
                let roles = &partial_guild.roles;

                // Get currently tracked user, channel and roles ids of the guild
                let user_ids: Vec<_> =
                    get_guild_ids!(database, guild_id, "users", "\"user\"", UserId);
                let channel_ids: Vec<_> =
                    get_guild_ids!(database, guild_id, "channels", "channel", ChannelId);
                let role_ids: Vec<_> = get_guild_ids!(database, guild_id, "roles", "role", RoleId);

                for user_id in user_ids {
                    let member = partial_guild.member(&ctx, user_id).await;

                    if matches!(member, Err(_)) {
                        // Delete user from the database
                        database
                            .client
                            .execute(
                                "DELETE FROM users WHERE guild = $1 AND \"user\" = $2",
                                &[&(guild_id.0 as i64), &(user_id.0 as i64)],
                            )
                            .await
                            .unwrap();
                    }
                }

                for channel_id in channel_ids {
                    if !channels.contains_key(&channel_id) {
                        // Delete channel from the database
                        database
                            .client
                            .execute(
                                "DELETE FROM channels WHERE guild = $1 AND channel = $2",
                                &[&(guild_id.0 as i64), &(channel_id.0 as i64)],
                            )
                            .await
                            .unwrap();
                    }
                }

                for role_id in role_ids {
                    if !roles.contains_key(&role_id) {
                        // Delete channel from the database
                        database
                            .client
                            .execute(
                                "DELETE FROM roles WHERE guild = $1 AND role = $2",
                                &[&(guild_id.0 as i64), &(role_id.0 as i64)],
                            )
                            .await
                            .unwrap();
                    }
                }

                // Get tracked message ids of the guild
                let message_ids: Vec<_> = {
                    let rows = database
                        .client
                        .query(
                            "SELECT channel, message FROM messages WHERE guild = $1",
                            &[&(guild_id.0 as i64)],
                        )
                        .await
                        .unwrap();

                    rows.iter()
                        .map(|row| {
                            (
                                ChannelId(row.get::<_, i64>(0) as u64),
                                MessageId(row.get::<_, i64>(1) as u64),
                            )
                        })
                        .collect()
                };

                for (channel_id, message_id) in message_ids {
                    let message = channel_id.message(&ctx.http, message_id).await;

                    if matches!(message, Err(_)) {
                        // Delete message from the database
                        database
                            .client
                            .execute(
                                "
                                DELETE FROM messages
                                WHERE guild = $1 AND channel = $2 AND message = $3
                                ",
                                &[
                                    &(guild_id.0 as i64),
                                    &(channel_id.0 as i64),
                                    &(message_id.0 as i64),
                                ],
                            )
                            .await
                            .unwrap();
                    }
                }
            }
            // Bot is not on the guild anymore
            _ => {
                // Delete guild from the database
                database
                    .client
                    .execute(
                        "DELETE FROM guilds WHERE guild = $1",
                        &[&(guild_id.0 as i64)],
                    )
                    .await
                    .unwrap();
            }
        }
    }
}
