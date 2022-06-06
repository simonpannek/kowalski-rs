use itertools::Itertools;
use serenity::client::Context;
use serenity::model::id::{ChannelId, GuildId, MessageId};

use crate::data;
use crate::database::client::Database;
use crate::error::KowalskiError;

pub async fn message_delete(
    ctx: &Context,
    channel_id: ChannelId,
    deleted_message_id: MessageId,
    guild_id: Option<GuildId>,
) -> Result<(), KowalskiError> {
    if let Some(guild_id) = guild_id {
        // Get database
        let database = data!(ctx, Database);

        // Get guild, channel and message ids
        let guild_db_id = guild_id.0 as i64;
        let channel_db_id = channel_id.0 as i64;
        let message_db_id = deleted_message_id.0 as i64;

        database
            .client
            .execute(
                "
                DELETE FROM messages
                WHERE guild = $1::BIGINT AND channel = $2::BIGINT AND message = $3::BIGINT
                ",
                &[&guild_db_id, &channel_db_id, &message_db_id],
            )
            .await?;
    }

    Ok(())
}

pub async fn message_delete_bulk(
    ctx: &Context,
    channel_id: ChannelId,
    deleted_messages_ids: Vec<MessageId>,
    guild_id: Option<GuildId>,
) -> Result<(), KowalskiError> {
    if let Some(guild_id) = guild_id {
        // Get database
        let database = data!(ctx, Database);

        // Get guild, channel and message ids
        let guild_db_id = guild_id.0 as i64;
        let channel_db_id = channel_id.0 as i64;
        let message_db_ids: Vec<_> = deleted_messages_ids
            .iter()
            .map(|deleted_message_id| deleted_message_id.0 as i64)
            .collect();

        database
            .client
            .execute(
                &format!(
                    "
                DELETE FROM messages
                WHERE guild = $1::BIGINT AND channel = $2::BIGINT AND message IN ({})
                ",
                    message_db_ids.iter().join(",")
                ),
                &[&guild_db_id, &channel_db_id],
            )
            .await?;
    }

    Ok(())
}
