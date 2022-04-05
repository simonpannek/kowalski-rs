use serde_json::Value;
use serenity::client::bridge::gateway::event::ShardStageUpdateEvent;
use serenity::model::channel::{
    Channel, ChannelCategory, GuildChannel, Message, PartialGuildChannel, StageInstance,
};
use serenity::model::event::{
    ChannelPinsUpdateEvent, GuildMemberUpdateEvent, GuildMembersChunkEvent, InviteCreateEvent,
    InviteDeleteEvent, MessageUpdateEvent, PresenceUpdateEvent, ResumedEvent, ThreadListSyncEvent,
    ThreadMembersUpdateEvent, TypingStartEvent, VoiceServerUpdateEvent,
};
use serenity::model::gateway::Presence;
use serenity::model::guild::{
    Emoji, Guild, GuildUnavailable, Integration, PartialGuild, Role, ThreadMember,
};
use serenity::model::id::{ApplicationId, EmojiId, IntegrationId, RoleId};
use serenity::model::interactions::application_command::ApplicationCommand;
use serenity::model::prelude::{CurrentUser, VoiceState};
use serenity::{
    async_trait,
    client::{Context, EventHandler},
    model::{
        channel::Reaction,
        gateway::Ready,
        guild::Member,
        id::{ChannelId, GuildId, MessageId},
        interactions::Interaction,
        user::User,
    },
};
use std::collections::HashMap;
use tracing::error;

use crate::{
    events::{
        guild_member_removal::guild_member_removal,
        interaction_create::interaction_create,
        reaction::{reaction_add, reaction_remove, reaction_remove_all},
        ready::ready,
    },
    strings::{ERR_MEMBER_REMOVAL, ERR_REACTION},
};

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
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
