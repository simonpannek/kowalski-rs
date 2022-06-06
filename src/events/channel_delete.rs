use serenity::client::Context;
use serenity::model::channel::GuildChannel;

use crate::data;
use crate::database::client::Database;
use crate::error::KowalskiError;

pub async fn channel_delete(ctx: &Context, channel: &GuildChannel) -> Result<(), KowalskiError> {
    // Get database
    let database = data!(ctx, Database);

    // Get guild and channel ids
    let guild_db_id = channel.guild_id.0 as i64;
    let channel_db_id = channel.id.0 as i64;

    database
        .client
        .execute(
            "
            DELETE FROM channels
            WHERE guild = $1::BIGINT AND channel = $2::BIGINT
            ",
            &[&guild_db_id, &channel_db_id],
        )
        .await?;

    Ok(())
}
