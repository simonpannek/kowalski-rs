use itertools::Itertools;
use serenity::{
    client::Context,
    model::{id::ChannelId, interactions::application_command::ApplicationCommandInteraction},
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

    // Get all channels where the channel is activated
    let channels: Vec<_> = {
        let rows = database
            .client
            .query(
                "SELECT channel FROM score_drops WHERE guild = $1",
                &[&guild_db_id],
            )
            .await?;

        rows.iter()
            .map(|row| ChannelId(row.get::<_, i64>(0) as u64))
            .collect()
    };

    let channels = channels
        .iter()
        .map(|&channel_id| channel_id.mention())
        .join(", ");

    if channels.is_empty() {
        send_response(
            &ctx,
            &command,
            &command_config,
            "Drops",
            "Drops are currently not activated for this guild.",
        )
        .await
    } else {
        send_response(
            &ctx,
            &command,
            &command_config,
            "Drops",
            &format!(
                "Drops are currently activated for the following channels: {}",
                channels
            ),
        )
        .await
    }
}
