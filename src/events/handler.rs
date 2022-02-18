use serenity::{
    async_trait,
    client::{Context, EventHandler},
    model::{gateway::Ready, interactions::Interaction},
};

use crate::events::{interaction_create::interaction_create, ready::ready};

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, rdy: Ready) {
        ready(&ctx, rdy).await
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        interaction_create(&ctx, interaction).await
    }
}
