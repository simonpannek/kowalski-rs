use serenity::model::id::{ChannelId, MessageId, RoleId, UserId};
use serenity::{
    client::Context,
    model::{
        gateway::{Activity, Ready},
        id::GuildId,
        interactions::application_command::ApplicationCommand,
    },
};
use std::time::Duration;
use tracing::info;

use crate::{
    config::Config,
    data,
    database::{client::Database, types::ModuleStatus},
    reminders::check_reminders,
    strings::{ERR_CMD_CREATION, ERR_DB_QUERY, INFO_CMD_GLOBAL, INFO_CMD_MODULE, INFO_CONNECTED},
    utils::{create_command, create_module_command},
};

pub async fn ready(ctx: &Context, rdy: Ready) {
    info!("{}", INFO_CONNECTED);

    // Set the bot status
    let activity = Activity::listening("reactions");
    ctx.set_activity(activity).await;

    // Clean all the database tables
    clean_database(ctx).await;

    // Repeatedly check for reminders
    check_reminders(ctx.clone(), Duration::from_secs(60));

    setup_commands(ctx, rdy).await;
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
                                "DELETE FROM users WHERE guild = $1 AND channel = $2 message = $3",
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

async fn setup_commands(ctx: &Context, _rdy: Ready) {
    // Get config and database
    let (config, database) = data!(ctx, (Config, Database));

    // Create global commands
    create_global_commands(ctx, &config).await;
    info!("{}", INFO_CMD_GLOBAL);

    // Create module commands per guild
    create_module_commands(ctx, &config, &database).await;
    info!("{}", INFO_CMD_MODULE);
}

async fn create_global_commands(ctx: &Context, config: &Config) -> Vec<ApplicationCommand> {
    // Get commands without a module
    let filtered = config
        .commands
        .iter()
        .filter(|(_, options)| options.module.is_none());

    ApplicationCommand::set_global_application_commands(&ctx.http, |commands| {
        for (name, options) in filtered {
            let command = create_command(name, options);
            commands.add_application_command(command);
        }

        commands
    })
    .await
    .expect(ERR_CMD_CREATION)
}

async fn create_module_commands(ctx: &Context, config: &Config, database: &Database) {
    let modules = database
        .client
        .query("SELECT * FROM modules", &[])
        .await
        .expect(ERR_DB_QUERY);

    for row in modules {
        let guild = GuildId(row.get::<_, i64>(0) as u64);
        let status: ModuleStatus = row.get(1);

        create_module_command(ctx, config, guild, &status).await
    }
}
