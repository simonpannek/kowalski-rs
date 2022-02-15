use std::error::Error;

use tracing::error;

use kowalski_rs::{client::Client, strings::ERR_CLIENT};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    // Create kowalski
    let mut kowalski = Client::default().await?;

    // Start kowalski
    if let Err(why) = kowalski.start().await {
        error!("{}: {}", ERR_CLIENT, why);
    }

    Ok(())
}
