use chrono::Utc;
use serenity::{
    client::Context,
    model::{channel::Message, interactions::application_command::ApplicationCommandInteraction},
};

use crate::{
    config::Command,
    error::ExecutionError,
    strings::ERR_API_LOAD,
    utils::{parse_arg, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), ExecutionError> {
    let options = &command.data.options;

    // Parse argument
    let count = parse_arg(options, 0)?;

    // Get message to to start deleting from
    let start = command.get_interaction_response(&ctx.http).await?;

    // Get messages to delete
    let messages = command
        .channel_id
        .messages(&ctx.http, |builder| builder.before(start.id).limit(count))
        .await?;

    let filtered: Vec<&Message> = messages
        .iter()
        .filter(|message| {
            let age_weeks = Utc::now()
                .signed_duration_since(message.timestamp)
                .num_days();

            age_weeks < 14
        })
        .collect();

    let title = format!("Delete {} messages", count);

    match filtered.len() {
        0 => {
            send_response(
                ctx,
                command,
                command_config,
                &title,
                "I couldn't find any messages to delete.",
            )
            .await
        }
        1 => {
            // Get the single message
            let message = filtered.get(0).ok_or(ExecutionError::new(ERR_API_LOAD))?;

            // Delete the message
            command
                .channel_id
                .delete_message(&ctx.http, message.id)
                .await?;

            send_response(
                ctx,
                command,
                command_config,
                &title,
                "I have deleted one message.",
            )
            .await
        }
        count => {
            // Delete the messages
            command
                .channel_id
                .delete_messages(&ctx.http, filtered.iter())
                .await?;

            send_response(
                ctx,
                command,
                command_config,
                &title,
                &format!("I have deleted {} messages.", count),
            )
            .await
        }
    }
}
