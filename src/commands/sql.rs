use std::borrow::Cow;

use serenity::{
    client::Context, http::AttachmentType,
    model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{
    config::{Command, Config},
    database::{client::Database, types::TableResolved},
    error::ExecutionError,
    history::History,
    strings::{ERR_API_LOAD, ERR_DATA_ACCESS},
    utils::{edit_response, parse_arg, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), ExecutionError> {
    // Get config, database and lock to history
    let (config, database, history_lock) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().expect(ERR_DATA_ACCESS).clone();
        let database = data.get::<Database>().expect(ERR_DATA_ACCESS).clone();
        let history_lock = data.get::<History>().expect(ERR_DATA_ACCESS).clone();

        (config, database, history_lock)
    };

    let options = &command.data.options;

    // Parse argument
    let option_name = &options
        .get(0)
        .ok_or(ExecutionError::new(ERR_API_LOAD))?
        .name;
    let query = parse_arg(options, 0)?;

    send_response(
        ctx,
        command,
        command_config,
        &format!("`{}`", query),
        "Executing SQL query...",
    )
    .await?;

    // Execute SQL query
    let result = database.client.query(query, &[]).await?;
    let resolved = TableResolved::new(ctx, result).await;

    // Add query to history
    {
        let mut history = history_lock.write().await;

        history.add_entry(
            &config,
            command.user.id,
            &command.data.name,
            option_name,
            query,
        );
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

    edit_response(
        ctx,
        command,
        command_config,
        &format!("`{}`", query),
        "I have executed the SQL query.",
    )
    .await
}
