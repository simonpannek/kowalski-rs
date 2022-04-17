use chrono::Utc;
use serenity::{
    client::Context,
    model::{channel::Message, interactions::application_command::ApplicationCommandInteraction},
};

use crate::{
    config::Command,
    error::KowalskiError,
    utils::{parse_arg, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    let options = &command.data.options;

    // Parse first argument
    let count = parse_arg(options, 0)?;

    let title = format!("Delete {} messages", count);

    // Get message to start deleting from
    let start = if options.len() > 1 {
        // Start deleting from the custom id given
        let start: u64 = parse_arg::<String>(options, 1)?.parse().unwrap();

        command.channel_id.message(&ctx.http, start).await
    } else {
        // Start deleting from the interaction response
        command.get_interaction_response(&ctx.http).await
    };

    match start {
        Ok(start) => {
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
                    let message = filtered.get(0).unwrap();

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
                        &format!(
                            "I have deleted {} messages going back from [here]({}).",
                            count,
                            start.link()
                        ),
                    )
                    .await
                }
            }
        }
        Err(_) => {
            send_response(
                ctx,
                command,
                command_config,
                &title,
                "I couldn't find the message to start deleting from.",
            )
            .await
        }
    }
}
