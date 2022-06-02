use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
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

    let pin_score: Option<i64> = database
        .client
        .query_opt(
            "
            SELECT score FROM score_auto_pin
            WHERE guild = $1::BIGINT",
            &[&guild_db_id],
        )
        .await?
        .map(|row| row.get(0));

    let delete_score: Option<i64> = database
        .client
        .query_opt(
            "
            SELECT score FROM score_auto_delete
            WHERE guild = $1::BIGINT",
            &[&guild_db_id],
        )
        .await?
        .map(|row| row.get(0));

    let mut content = format!("The following auto-moderation tools are available:\n\n");

    // Add auto pin information
    content.push_str("**Auto Pin:** ");

    match pin_score {
        Some(pin_score) => content.push_str(&format!(
            "I will automatically pin messages when the reach a score of {}.",
            pin_score
        )),
        None => content.push_str("Disabled"),
    };

    // Add auto delete information
    content.push_str("**Auto Delete:** ");

    match delete_score {
        Some(delete_score) => content.push_str(&format!(
            "I will automatically delete messages when the reach a score of {}.",
            delete_score
        )),
        None => content.push_str("Disabled"),
    };

    send_response(&ctx, &command, &command_config, "Auto-moderation", &content).await
}
