use serenity::{
    client::Context,
    model::{
        channel::Reaction,
        id::{ChannelId, MessageId},
    },
};

pub async fn reaction_add(_ctx: &Context, _add_reaction: Reaction) {
    todo!()
}

pub async fn reaction_remove(_ctx: &Context, _removed_reaction: Reaction) {
    todo!()
}

pub async fn reaction_remove_all(
    _ctx: Context,
    _channel_id: ChannelId,
    _removed_from_message_id: MessageId,
) {
    todo!()
}
