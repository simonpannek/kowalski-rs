use serenity::client::Context;
use serenity::model::guild::{Guild, UnavailableGuild};

use crate::data;
use crate::database::client::Database;
use crate::error::KowalskiError;

pub async fn guild_delete(
    ctx: &Context,
    incomplete: UnavailableGuild,
    _full: Option<Guild>,
) -> Result<(), KowalskiError> {
    // Check whether the bot was actually removed
    if !incomplete.unavailable {
        // Get database
        let database = data!(ctx, Database);

        // Get guild id
        let guild_db_id = incomplete.id.0 as i64;

        database
            .client
            .execute(
                "DELETE FROM guilds WHERE guild = $1::BIGINT",
                &[&guild_db_id],
            )
            .await?;
    }

    Ok(())
}
