use serenity::{
    async_trait,
    client::{Context, EventHandler},
    model::gateway::Ready,
    model::prelude::Activity,
};
use tracing::info;

use crate::strings::INFO_CONNECTED;

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _: Ready) {
        info!("{}", INFO_CONNECTED);

        // Set the bot status
        let activity = Activity::listening("reactions");
        ctx.set_activity(activity).await;

        // Setup commands
        // TODO
    }
}

impl Handler {}
