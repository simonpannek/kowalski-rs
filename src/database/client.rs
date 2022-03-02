use std::{env, error::Error, sync::Arc};

use serenity::prelude::TypeMapKey;
use tokio_postgres::{Client, NoTls};
use tracing::{error, info};

use crate::strings::{ERR_DB_CONNECTION, ERR_ENV_NOT_SET, INFO_DB_CONNECTED, INFO_DB_SETUP};

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
                        guild       BIGINT PRIMARY KEY,
                        status      BIT(8) NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS guilds (
                        guild       BIGINT PRIMARY KEY
                    );
                ",
            )
            .await?;

        info!("{}", INFO_DB_SETUP);

        Ok(Database { client })
    }
}

impl TypeMapKey for Database {
    type Value = Arc<Database>;
}
