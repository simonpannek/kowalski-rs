use serenity::model::channel::ReactionType;
use std::{env, error::Error, sync::Arc};

use serenity::prelude::TypeMapKey;
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
                    CREATE TABLE IF NOT EXISTS modules (
                        guild           BIGINT PRIMARY KEY,
                        status          BIT(8) NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS emojis (
                        id              SERIAL PRIMARY KEY,
                        unicode         TEXT,
                        emoji_guild     BIGINT,
                        CONSTRAINT unicode_or_guild
                            CHECK ((unicode IS NULL) != (emoji_guild IS NULL))
                    );

                    CREATE TABLE IF NOT EXISTS score_auto_delete (
                        guild           BIGINT PRIMARY KEY,
                        score           BIGINT NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS score_auto_pin (
                        guild           BIGINT PRIMARY KEY,
                        score           BIGINT NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS score_cooldowns (
                        guild           BIGINT,
                        role            BIGINT,
                        cooldown        BIGINT NOT NULL,
                        PRIMARY KEY (guild, role)
                    );

                    CREATE TABLE IF NOT EXISTS score_drops (
                        guild           BIGINT,
                        channel         BIGINT,
                        PRIMARY KEY (guild, channel)
                    );

                    CREATE TABLE IF NOT EXISTS score_emojis (
                        guild           BIGINT,
                        emoji           INT,
                        upvote          BOOLEAN NOT NULL,
                        PRIMARY KEY (guild, emoji),
                        CONSTRAINT fk_emojis
                            FOREIGN KEY (emoji) REFERENCES emojis(id)
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
                        CONSTRAINT fk_emojis
                            FOREIGN KEY (emoji) REFERENCES emojis(id)
                    );

                    CREATE TABLE IF NOT EXISTS score_roles (
                        guild           BIGINT,
                        role            BIGINT,
                        score           BIGINT,
                        PRIMARY KEY (guild, role, score)
                    );

                    CREATE TABLE IF NOT EXISTS reaction_roles (
                        guild           BIGINT,
                        channel         BIGINT,
                        message         BIGINT,
                        emoji           INT,
                        role            BIGINT,
                        slots           INT,
                        PRIMARY KEY (guild, channel, message, emoji, role),
                        CONSTRAINT fk_emojis
                            FOREIGN KEY (emoji) REFERENCES emojis(id),
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
                        PRIMARY KEY (guild, channel, \"user\", time)
                    );

                    CREATE TABLE IF NOT EXISTS guilds (
                        guild           BIGINT PRIMARY KEY
                    );
                ",
            )
            .await?;

        info!("{}", INFO_DB_SETUP);

        Ok(Database { client })
    }

    /// Gets the id of an emoji given the reaction type.
    ///
    /// Note: If the emoji is not registered before, it will create a new row
    pub async fn get_emoji(&self, emoji: &ReactionType) -> Result<i32, KowalskiError> {
        let row = match emoji {
            ReactionType::Custom { id: emoji_id, .. } => {
                self.client
                    .query_one(
                        "
                        WITH id_row AS (
                            SELECT id FROM emojis
                            WHERE emoji_guild = $1::BIGINT
                        ), new_row AS (
                            INSERT INTO emojis (emoji_guild)
                            SELECT $1::BIGINT
                            WHERE NOT EXISTS (SELECT * FROM id_row)
                            RETURNING id
                        )

                        SELECT * FROM id_row
                        UNION ALL
                        SELECT * FROM new_row
                        ",
                        &[&(emoji_id.0 as i64)],
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
