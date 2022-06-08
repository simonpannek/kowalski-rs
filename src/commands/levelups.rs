use itertools::Itertools;
use serenity::{
    client::Context,
    model::{id::RoleId, interactions::application_command::ApplicationCommandInteraction},
    prelude::Mentionable,
};

use crate::{
    config::Command, data, database::client::Database, error::KowalskiError, utils::send_response,
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get database
    let database = data!(ctx, Database);

    let guild_id = command.guild_id.unwrap();

    // Get guild id
    let guild_db_id = database.get_guild(guild_id).await?;

    // Get roles and their respective cooldowns
    let role_cooldowns: Vec<_> = {
        let rows = database
            .client
            .query(
                "
                SELECT role, score FROM score_roles
                WHERE guild = $1::BIGINT
                ORDER BY score
                ",
                &[&guild_db_id],
            )
            .await?;

        rows.iter()
            .map(|row| (RoleId(row.get::<_, i64>(0) as u64), row.get::<_, i64>(1)))
            .collect()
    };

    let levelup_roles = role_cooldowns
        .iter()
        .map(|&(role_id, cooldown)| {
            format!(
                "{}: **score {} {}**",
                role_id.mention(),
                if cooldown >= 0 { ">=" } else { "<=" },
                cooldown
            )
        })
        .join("\n");

    let title = "Level-ups roles";

    if levelup_roles.is_empty() {
        send_response(
            &ctx,
            &command,
            &command_config,
            &title,
            "There are currently no level-up roles defined for this server.",
        )
        .await
    } else {
        send_response(
            &ctx,
            &command,
            &command_config,
            &title,
            &format!(
                "The following roles will get assigned to users when they reach a certain score:
                {}",
                levelup_roles
            ),
        )
        .await
    }
}
