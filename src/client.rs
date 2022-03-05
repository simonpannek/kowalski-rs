use std::{env, error::Error, sync::Arc};
use tokio::sync::RwLock;

use crate::cooldowns::Cooldowns;
use crate::history::History;
use crate::{
    config::Config, database::client::Database, events::handler::Handler, strings::ERR_ENV_NOT_SET,
};

/// The bot client.
pub struct Client {
    client: serenity::Client,
}

impl Client {
    pub async fn default() -> Result<Self, Box<dyn Error>> {
        // Get bot token
        let token = env::var("BOT_TOKEN").expect(&format!("{}: {}", ERR_ENV_NOT_SET, "BOT_TOKEN"));

        // Create the database
        Client::new(token).await
    }

    pub async fn new(token: String) -> Result<Self, Box<dyn Error>> {
        // Get bot application id
        let id = env::var("BOT_ID")
            .expect(&format!("{}: {}", ERR_ENV_NOT_SET, "BOT_ID"))
            .parse()?;

        // Build the database
        let client = serenity::Client::builder(token)
            .event_handler(Handler)
            .application_id(id)
            .await?;

        {
            let mut data = client.data.write().await;

            // Add config to data
            data.insert::<Config>(Arc::new(Config::new().await?));
            // Add database to data
            data.insert::<Database>(Arc::new(Database::new().await?));
            // Add query history to data
            data.insert::<History>(Arc::new(RwLock::new(History::new())));
            // Add cooldowns to data
            data.insert::<Cooldowns>(Arc::new(RwLock::new(Cooldowns::new())));
        }

        Ok(Client { client })
    }

    pub async fn start(&mut self) -> Result<(), serenity::Error> {
        self.client.start().await
    }
}
