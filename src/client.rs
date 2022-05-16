use serenity::prelude::GatewayIntents;
use std::{env, error::Error, sync::Arc};
use tokio::sync::RwLock;

#[cfg(feature = "nlp-model")]
use crate::model::Model;
use crate::{
    config::Config, cooldowns::Cooldowns, credits::Credits, database::client::Database,
    events::handler::Handler, history::History, strings::ERR_ENV_NOT_SET,
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
        let client = serenity::Client::builder(
            token,
            GatewayIntents::GUILD_MEMBERS
                | GatewayIntents::GUILD_MESSAGES
                | GatewayIntents::GUILD_MESSAGE_REACTIONS,
        )
        .event_handler(Handler)
        .application_id(id)
        .await?;

        {
            let mut data = client.data.write().await;

            // Add config to data
            data.insert::<Config>(Arc::new(Config::new().await?));
            // Add database to data
            data.insert::<Database>(Arc::new(Database::new().await?));
            // Add cooldowns to data
            data.insert::<Cooldowns>(Arc::new(RwLock::new(Cooldowns::new())));
            // Add credits to data
            data.insert::<Credits>(Arc::new(RwLock::new(Credits::new())));
            // Add query history to data
            data.insert::<History>(Arc::new(RwLock::new(History::new())));
            #[cfg(feature = "nlp-model")]
            // Add nlp model to data
            data.insert::<Model>(Arc::new(Model::new().await?));
        }

        Ok(Client { client })
    }

    pub async fn start(&mut self) -> Result<(), serenity::Error> {
        self.client.start().await
    }
}
