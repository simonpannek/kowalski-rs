use std::collections::HashMap;

use serenity::{
    client::Context,
    model::{
        guild::Emoji,
        id::{EmojiId, GuildId},
    },
};

use crate::{data, database::client::Database, error::KowalskiError};

pub async fn guild_emojis_update(
    ctx: &Context,
    guild_id: GuildId,
    current_state: HashMap<EmojiId, Emoji>,
) -> Result<(), KowalskiError> {
    // Get database
    let database = data!(ctx, Database);

    // Get guild id
    let guild_db_id = guild_id.0 as i64;

    // Get all emojis tracked by the database for this guild
    let emoji_ids: Vec<_> = {
        let rows = database
            .client
            .query(
                "SELECT guild_emoji FROM emojis WHERE guild = $1::BIGINT",
                &[&guild_db_id],
            )
            .await?;

        rows.iter()
            .map(|row| EmojiId(row.get::<_, i64>(0) as u64))
            .collect()
    };

    for emoji_id in emoji_ids {
        // Check whether emoji still exists
        if !current_state.contains_key(&emoji_id) {
            let emoji_db_id = emoji_id.0 as i64;

            // Delete the emoji
            database
                .client
                .execute(
                    "
            DELETE FROM emojis
            WHERE guild = $1::BIGINT AND guild_emoji = $2::BIGINT
            ",
                    &[&guild_db_id, &emoji_db_id],
                )
                .await?;
        }
    }

    Ok(())
}
