use std::{env, error::Error, sync::Arc};

use serenity::{
    model::{
        channel::ReactionType,
        id::{ChannelId, GuildId, MessageId, RoleId, UserId},
    },
    prelude::TypeMapKey,
};
use tokio_postgres::{Client, NoTls};
use tracing::{error, info};

use crate::{
    error::KowalskiError,
    strings::{ERR_DB_CONNECTION, ERR_ENV_NOT_SET, INFO_DB_CONNECTED, INFO_DB_SETUP},
};

/// The database client.
pub struct Database {
    pub client: Client,
}

impl Database {
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        // Get database config (https://docs.rs/tokio-postgres/0.7.2/tokio_postgres/config/struct.Config.html)
        let config = env::var("DB_CONF").expect(&format!("{}: {}", ERR_ENV_NOT_SET, "DB_CONF"));

        // Connect to the database
        let (client, connection) = tokio_postgres::connect(&config, NoTls).await?;

        // Handle database events on an extra thread
        tokio::spawn(async move {
            if let Err(why) = connection.await {
                error!("{}: {}", ERR_DB_CONNECTION, why);
            }
        });
        info!("{}", INFO_DB_CONNECTED);

        // Create tables if they do not exist yet
        client
            .batch_execute(
                "
                    CREATE TABLE IF NOT EXISTS guilds (
                        guild           BIGINT PRIMARY KEY
                    );

                    CREATE TABLE IF NOT EXISTS users (
                        guild           BIGINT,
                        \"user\"        BIGINT,
                        PRIMARY KEY (guild, \"user\"),
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS channels (
                        guild           BIGINT,
                        channel         BIGINT,
                        PRIMARY KEY (guild, channel),
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS roles (
                        guild           BIGINT,
                        role            BIGINT,
                        PRIMARY KEY (guild, role),
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS messages (
                        guild           BIGINT,
                        channel         BIGINT,
                        message         BIGINT,
                        PRIMARY KEY (guild, channel, message),
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_channels
                            FOREIGN KEY (guild, channel)
                            REFERENCES channels(guild, channel)
                            ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS emojis (
                        id              SERIAL PRIMARY KEY,
                        unicode         TEXT,
                        guild           BIGINT,
                        guild_emoji     BIGINT,
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE,
                        CONSTRAINT unicode_or_guild
                            CHECK ((guild IS NULL) = (guild_emoji IS NULL)
                            AND (unicode IS NULL) != (guild_emoji IS NULL))
                    );

                    CREATE TABLE IF NOT EXISTS modules (
                        guild           BIGINT PRIMARY KEY,
                        status          BIT(8) NOT NULL,
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS score_auto_delete (
                        guild           BIGINT PRIMARY KEY,
                        score           BIGINT NOT NULL,
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS score_auto_pin (
                        guild           BIGINT PRIMARY KEY,
                        score           BIGINT NOT NULL,
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS score_cooldowns (
                        guild           BIGINT,
                        role            BIGINT,
                        cooldown        BIGINT NOT NULL,
                        PRIMARY KEY (guild, role),
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_roles
                            FOREIGN KEY (guild, role)
                            REFERENCES roles(guild, role)
                            ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS score_drops (
                        guild           BIGINT,
                        channel         BIGINT,
                        PRIMARY KEY (guild, channel),
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_channels
                            FOREIGN KEY (guild, channel)
                            REFERENCES channels(guild, channel)
                            ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS score_emojis (
                        guild           BIGINT,
                        emoji           INT,
                        upvote          BOOLEAN NOT NULL,
                        PRIMARY KEY (guild, emoji),
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_emojis
                            FOREIGN KEY (emoji)
                            REFERENCES emojis(id)
                            ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS score_reactions (
                        guild           BIGINT,
                        user_from       BIGINT,
                        user_to         BIGINT,
                        channel         BIGINT,
                        message         BIGINT,
                        emoji           INT,
                        native          BOOLEAN NOT NULL DEFAULT true,
                        PRIMARY KEY (guild, user_from, user_to, channel, message, emoji),
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_users
                            FOREIGN KEY (guild, user_to)
                            REFERENCES users(guild, \"user\")
                            ON DELETE CASCADE,
                        CONSTRAINT fk_score_emojis
                            FOREIGN KEY (guild, emoji)
                            REFERENCES score_emojis(guild, emoji)
                            ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS score_roles (
                        guild           BIGINT,
                        role            BIGINT,
                        score           BIGINT,
                        PRIMARY KEY (guild, role, score),
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_roles
                            FOREIGN KEY (guild, role)
                            REFERENCES roles(guild, role)
                            ON DELETE CASCADE
                    );

                    CREATE TABLE IF NOT EXISTS reaction_roles (
                        guild           BIGINT,
                        channel         BIGINT,
                        message         BIGINT,
                        emoji           INT,
                        role            BIGINT,
                        slots           INT,
                        PRIMARY KEY (guild, channel, message, emoji, role),
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_channels
                            FOREIGN KEY (guild, channel)
                            REFERENCES channels(guild, channel)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_messages
                            FOREIGN KEY (guild, channel, message)
                            REFERENCES messages(guild, channel, message)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_emojis
                            FOREIGN KEY (emoji)
                            REFERENCES emojis(id)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_roles
                            FOREIGN KEY (guild, role)
                            REFERENCES roles(guild, role)
                            ON DELETE CASCADE,
                        CONSTRAINT unsigned_slots
                            CHECK (slots >= 0)
                    );

                    CREATE TABLE IF NOT EXISTS reminders (
                        guild           BIGINT,
                        channel         BIGINT,
                        message         BIGINT,
                        \"user\"        BIGINT,
                        time            TIMESTAMP WITH TIME ZONE,
                        content         TEXT NOT NULL,
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_channels
                            FOREIGN KEY (guild, channel)
                            REFERENCES channels(guild, channel)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_messages
                            FOREIGN KEY (guild, channel, message)
                            REFERENCES messages(guild, channel, message)
                            ON DELETE CASCADE,
                        CONSTRAINT fk_users
                            FOREIGN KEY (guild, \"user\")\
                            REFERENCES users(guild, \"user\")
                            ON DELETE CASCADE,
                        PRIMARY KEY (guild, channel, \"user\", time)
                    );

                    CREATE TABLE IF NOT EXISTS owned_guilds (
                        guild           BIGINT PRIMARY KEY,
                        CONSTRAINT fk_guilds
                            FOREIGN KEY (guild)
                            REFERENCES guilds(guild)
                            ON DELETE CASCADE
                    );
                ",
            )
            .await?;

        info!("{}", INFO_DB_SETUP);

        Ok(Database { client })
    }

    /// Gets the id of a guild given the GuildId object.
    ///
    /// Note: If the guild is not registered before, it will create a new row
    pub async fn get_guild(&self, guild_id: GuildId) -> Result<i64, KowalskiError> {
        self.client
            .execute(
                "
            WITH duplicate AS (
                SELECT * FROM guilds
                WHERE guild = $1::BIGINT
            )

            INSERT INTO guilds
            SELECT $1::BIGINT
            WHERE NOT EXISTS (SELECT * FROM duplicate)
            ",
                &[&(guild_id.0 as i64)],
            )
            .await?;

        Ok(guild_id.0 as i64)
    }

    /// Gets the id of a user given the GuildId and UserId object.
    ///
    /// Note: If the guild or user is not registered before, it will create new rows
    pub async fn get_user(&self, guild_id: GuildId, user_id: UserId) -> Result<i64, KowalskiError> {
        let guild_db_id = self.get_guild(guild_id).await?;

        self.client
            .execute(
                "
            WITH duplicate AS (
                SELECT * FROM users
                WHERE guild = $1::BIGINT AND \"user\" = $2::BIGINT
            )

            INSERT INTO users
            SELECT $1::BIGINT, $2::BIGINT
            WHERE NOT EXISTS (SELECT * FROM duplicate)
            ",
                &[&guild_db_id, &(user_id.0 as i64)],
            )
            .await?;

        Ok(user_id.0 as i64)
    }

    /// Gets the id of a role given the GuildId and RoleId object.
    ///
    /// Note: If the guild or role is not registered before, it will create new rows
    pub async fn get_role(&self, guild_id: GuildId, role_id: RoleId) -> Result<i64, KowalskiError> {
        let guild_db_id = self.get_guild(guild_id).await?;

        self.client
            .execute(
                "
            WITH duplicate AS (
                SELECT * FROM roles
                WHERE guild = $1::BIGINT AND role = $2::BIGINT
            )

            INSERT INTO roles
            SELECT $1::BIGINT, $2::BIGINT
            WHERE NOT EXISTS (SELECT * FROM duplicate)
            ",
                &[&guild_db_id, &(role_id.0 as i64)],
            )
            .await?;

        Ok(role_id.0 as i64)
    }

    /// Gets the id of a channel given the GuildId and ChannelId object.
    ///
    /// Note: If the guild or role is not registered before, it will create new rows
    pub async fn get_channel(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
    ) -> Result<i64, KowalskiError> {
        let guild_db_id = self.get_guild(guild_id).await?;

        self.client
            .execute(
                "
            WITH duplicate AS (
                SELECT * FROM channels
                WHERE guild = $1::BIGINT AND channel = $2::BIGINT
            )

            INSERT INTO channels
            SELECT $1::BIGINT, $2::BIGINT
            WHERE NOT EXISTS (SELECT * FROM duplicate)
            ",
                &[&guild_db_id, &(channel_id.0 as i64)],
            )
            .await?;

        Ok(channel_id.0 as i64)
    }

    /// Gets the id of a message given the GuildId and MessageId object.
    ///
    /// Note: If the guild or role is not registered before, it will create new rows
    pub async fn get_message(
        &self,
        guild_id: GuildId,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> Result<i64, KowalskiError> {
        let guild_db_id = self.get_guild(guild_id).await?;
        let channel_db_id = self.get_channel(guild_id, channel_id).await?;

        self.client
            .execute(
                "
            WITH duplicate AS (
                SELECT * FROM messages
                WHERE guild = $1::BIGINT AND channel = $2::BIGINT AND message = $3::BIGINT
            )

            INSERT INTO messages
            SELECT $1::BIGINT, $2::BIGINT, $3::BIGINT
            WHERE NOT EXISTS (SELECT * FROM duplicate)
            ",
                &[&guild_db_id, &channel_db_id, &(message_id.0 as i64)],
            )
            .await?;

        Ok(message_id.0 as i64)
    }

    /// Gets the id of an emoji given the reaction type.
    ///
    /// Note: If the emoji is not registered before, it will create a new row
    pub async fn get_emoji(
        &self,
        guild_id: GuildId,
        emoji: &ReactionType,
    ) -> Result<i32, KowalskiError> {
        let row = match emoji {
            ReactionType::Custom { id: emoji_id, .. } => {
                // Get guild id
                let guild_db_id = self.get_guild(guild_id).await?;

                self.client
                    .query_one(
                        "
                        WITH id_row AS (
                            SELECT id FROM emojis
                            WHERE guild = $1::BIGINT AND guild_emoji = $2::BIGINT
                        ), new_row AS (
                            INSERT INTO emojis (guild, guild_emoji)
                            SELECT $1::BIGINT, $2::BIGINT
                            WHERE NOT EXISTS (SELECT * FROM id_row)
                            RETURNING id
                        )

                        SELECT * FROM id_row
                        UNION ALL
                        SELECT * FROM new_row
                        ",
                        &[&guild_db_id, &(emoji_id.0 as i64)],
                    )
                    .await?
            }
            ReactionType::Unicode(string) => {
                self.client
                    .query_one(
                        "
                        WITH id_row AS (
                            SELECT id FROM emojis
                            WHERE unicode = $1::TEXT
                        ), new_row AS (
                            INSERT INTO emojis (unicode)
                            SELECT $1::TEXT
                            WHERE NOT EXISTS (SELECT * FROM id_row)
                            RETURNING id
                        )

                        SELECT * FROM id_row
                        UNION ALL
                        SELECT * FROM new_row
                        ",
                        &[string],
                    )
                    .await?
            }
            _ => unreachable!(),
        };

        Ok(row.get(0))
    }
}

impl TypeMapKey for Database {
    type Value = Arc<Database>;
}
