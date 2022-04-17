use std::borrow::Cow;

use serenity::{
    client::Context, http::AttachmentType,
    model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{
    config::{Command, Config},
    data,
    database::{client::Database, types::TableResolved},
    error::KowalskiError,
    history::History,
    utils::{parse_arg, parse_arg_name, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get config, database and lock to history
    let (config, database, history_lock) = data!(ctx, (Config, Database, History));

    let options = &command.data.options;

    // Parse argument
    let query = parse_arg(options, 0)?;

    // Execute SQL query
    let result = database.client.query(query, &[]).await?;
    let resolved = TableResolved::new(ctx, result).await;

    // Add query to history
    {
        let mut history = history_lock.write().await;

        history.add_entry(&config, command.user.id, parse_arg_name(options, 0)?, query);
    }

    let string = resolved.table(0, resolved.len()).to_string();

    if !string.is_empty() {
        let file = AttachmentType::Bytes {
            data: Cow::from(string.as_bytes()),
            filename: "result.txt".to_string(),
        };

        command
            .channel_id
            .send_message(&ctx.http, |message| message.add_file(file))
            .await?;
    }

    send_response(
        ctx,
        command,
        command_config,
        &format!("`{}`", query),
        "I have executed the SQL query.",
    )
    .await
}
