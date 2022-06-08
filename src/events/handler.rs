use std::collections::HashMap;

use serenity::{
    async_trait,
    client::{Context, EventHandler},
    model::{
        channel::{GuildChannel, Reaction},
        gateway::Ready,
        guild::{Emoji, Guild, Member, Role, UnavailableGuild},
        id::{ChannelId, EmojiId, GuildId, MessageId, RoleId},
        interactions::Interaction,
        user::User,
    },
};
use tracing::error;

use crate::{
    events::{
        channel_delete::channel_delete,
        guild_delete::guild_delete,
        guild_emojis_update::guild_emojis_update,
        guild_member_removal::guild_member_removal,
        guild_role_delete::guild_role_delete,
        interaction_create::interaction_create,
        message_delete::{message_delete, message_delete_bulk},
        reaction::{reaction_add, reaction_remove, reaction_remove_all},
        ready::ready,
    },
    strings::{ERR_MEMBER_REMOVAL, ERR_REACTION},
};

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn channel_delete(&self, ctx: Context, channel: &GuildChannel) {
        channel_delete(&ctx, channel).await.unwrap()
    }

    async fn guild_delete(&self, ctx: Context, incomplete: UnavailableGuild, full: Option<Guild>) {
        guild_delete(&ctx, incomplete, full).await.unwrap()
    }

    async fn guild_emojis_update(
        &self,
        ctx: Context,
        guild_id: GuildId,
        current_state: HashMap<EmojiId, Emoji>,
    ) {
        guild_emojis_update(&ctx, guild_id, current_state)
            .await
            .unwrap()
    }

    async fn guild_member_removal(
        &self,
        ctx: Context,
        guild_id: GuildId,
        user: User,
        member_data: Option<Member>,
    ) {
        if let Err(why) = guild_member_removal(&ctx, guild_id, user, member_data).await {
            error!("{}: {:?}", ERR_MEMBER_REMOVAL, why);
        }
    }

    async fn guild_role_delete(
        &self,
        ctx: Context,
        guild_id: GuildId,
        removed_role_id: RoleId,
        removed_role_data_if_available: Option<Role>,
    ) {
        guild_role_delete(
            &ctx,
            guild_id,
            removed_role_id,
            removed_role_data_if_available,
        )
        .await
        .unwrap()
    }

    async fn message_delete(
        &self,
        ctx: Context,
        channel_id: ChannelId,
        deleted_message_id: MessageId,
        guild_id: Option<GuildId>,
    ) {
        message_delete(&ctx, channel_id, deleted_message_id, guild_id)
            .await
            .unwrap()
    }

    async fn message_delete_bulk(
        &self,
        ctx: Context,
        channel_id: ChannelId,
        multiple_deleted_messages_ids: Vec<MessageId>,
        guild_id: Option<GuildId>,
    ) {
        message_delete_bulk(&ctx, channel_id, multiple_deleted_messages_ids, guild_id)
            .await
            .unwrap()
    }

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
