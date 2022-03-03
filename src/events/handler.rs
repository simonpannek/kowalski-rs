use serenity::{
    async_trait,
    client::{Context, EventHandler},
    model::{
        channel::Reaction,
        gateway::Ready,
        id::{ChannelId, MessageId},
        interactions::Interaction,
    },
};
use tracing::error;

use crate::{
    events::{
        interaction_create::interaction_create,
        reaction::{reaction_add, reaction_remove, reaction_remove_all},
        ready::ready,
    },
    strings::ERR_REACTION,
};

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn reaction_add(&self, ctx: Context, add_reaction: Reaction) {
        if let Err(why) = reaction_add(&ctx, add_reaction).await {
            error!("{}: {:?}", ERR_REACTION, why);
        }
    }

    async fn reaction_remove(&self, ctx: Context, removed_reaction: Reaction) {
        if let Err(why) = reaction_remove(&ctx, removed_reaction).await {
            error!("{}: {:?}", ERR_REACTION, why);
        }
    }

    async fn reaction_remove_all(
        &self,
        ctx: Context,
        channel_id: ChannelId,
        removed_from_message_id: MessageId,
    ) {
        if let Err(why) = reaction_remove_all(ctx, channel_id, removed_from_message_id).await {
            error!("{}: {:?}", ERR_REACTION, why);
        }
    }

    async fn ready(&self, ctx: Context, rdy: Ready) {
        ready(&ctx, rdy).await
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        interaction_create(&ctx, interaction).await
    }
}
