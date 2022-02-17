use serenity::{
    async_trait,
    client::{Context, EventHandler},
    model::gateway::Ready,
};

use crate::events::ready::ready;

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, rdy: Ready) {
        ready(&ctx, rdy).await
    }
}
