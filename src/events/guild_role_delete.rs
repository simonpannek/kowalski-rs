use serenity::{
    client::Context,
    model::{
        guild::Role,
        id::{GuildId, RoleId},
    },
};

use crate::{data, database::client::Database, error::KowalskiError};

pub async fn guild_role_delete(
    ctx: &Context,
    guild_id: GuildId,
    removed_role_id: RoleId,
    _removed_role_data_if_available: Option<Role>,
) -> Result<(), KowalskiError> {
    // Get database
    let database = data!(ctx, Database);

    // Get guild and channel ids
    let guild_db_id = guild_id.0 as i64;
    let role_db_id = removed_role_id.0 as i64;

    database
        .client
        .execute(
            "
            DELETE FROM roles
            WHERE guild = $1::BIGINT AND role = $2::BIGINT
            ",
            &[&guild_db_id, &role_db_id],
        )
        .await?;

    Ok(())
}
