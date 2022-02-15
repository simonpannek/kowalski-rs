use std::{env, error::Error};

use crate::{events::handler::Handler, strings::ERR_ENV_NOT_SET};

pub struct Client {
    client: serenity::Client,
}

impl Client {
    pub async fn default() -> Result<Client, Box<dyn Error>> {
        // Get bot token
        let token = env::var("BOT_TOKEN").expect(&format!("{}: {}", ERR_ENV_NOT_SET, "BOT_TOKEN"));

        // Create the client
        Client::new(token).await
    }

    pub async fn new(token: String) -> Result<Client, Box<dyn Error>> {
        // Get bot application id
        let id = env::var("BOT_ID")
            .expect(&format!("{}: {}", ERR_ENV_NOT_SET, "BOT_ID"))
            .parse()?;

        // Build the client
        let client = serenity::Client::builder(token)
            .event_handler(Handler)
            .application_id(id)
            .await?;

        {
            // Add maps to the client data
            let _ = client.data.write().await;
            // TODO
        }

        Ok(Client { client })
    }

    pub async fn start(&mut self) -> Result<(), serenity::Error> {
        self.client.start().await
    }
}
