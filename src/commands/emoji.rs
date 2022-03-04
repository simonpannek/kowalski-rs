use std::{
    fmt::{Display, Formatter},
    str::FromStr,
    time::Duration,
};

use serenity::{
    client::Context,
    model::{
        channel::ReactionType, interactions::application_command::ApplicationCommandInteraction,
    },
    utils::parse_emoji,
};
use unic_emoji_char::is_emoji;

use crate::{
    config::{Command, Config},
    database::client::Database,
    error::ExecutionError,
    strings::{ERR_API_LOAD, ERR_CMD_ARGS_INVALID, ERR_DATA_ACCESS},
    utils::{parse_arg, send_confirmation, send_response, InteractionResponse},
};

enum Action {
    AddUpvote,
    AddDownvote,
    Remove,
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Action::AddUpvote => "Add upvote",
            Action::AddDownvote => "Add downvote",
            Action::Remove => "Remove",
        };

        write!(f, "{}", name)
    }
}

impl FromStr for Action {
    type Err = ExecutionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "add upvote" => Ok(Action::AddUpvote),
            "add downvote" => Ok(Action::AddDownvote),
            "remove" => Ok(Action::Remove),
            _ => Err(ExecutionError::new(&format!(
                "{}: {}",
                ERR_CMD_ARGS_INVALID, s
            ))),
        }
    }
}

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), ExecutionError> {
    // Get config and database
    let (config, database) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().expect(ERR_DATA_ACCESS).clone();
        let database = data.get::<Database>().expect(ERR_DATA_ACCESS).clone();

        (config, database)
    };

    let guild = command.guild_id.ok_or(ExecutionError::new(ERR_API_LOAD))?;

    let options = &command.data.options;

    // Parse arguments
    let action = Action::from_str(parse_arg(options, 0)?)?;
    let emoji = {
        let string: String = parse_arg(options, 1)?;
        match parse_emoji(&string) {
            Some(identifier) => guild
                .emoji(&ctx.http, identifier.id)
                .await
                .map_or(None, |emoji| {
                    Some(ReactionType::Custom {
                        animated: emoji.animated,
                        id: emoji.id,
                        name: Some(emoji.name),
                    })
                }),
            None => {
                let chars: Vec<char> = string.chars().collect();
                let first = chars.get(0);

                if chars.len() == 1 && is_emoji(*first.unwrap()) {
                    Some(ReactionType::Unicode(first.unwrap().to_string()))
                } else {
                    None
                }
            }
        }
    };

    let title = format!("{} emoji", action);

    match emoji {
        Some(emoji) => {
            // Get the id of the emoji in the emoji table
            let emoji_id = database.get_emoji(&emoji).await?;

            match action {
                Action::AddUpvote | Action::AddDownvote => {
                    let upvote = matches!(action, Action::AddUpvote);

                    // Insert entry
                    database
                        .client
                        .execute(
                            "
                    INSERT INTO reaction_emojis
                    VALUES ($1::BIGINT, $2::INT, $3::BOOLEAN)
                    ",
                            &[&i64::from(guild), &emoji_id, &upvote],
                        )
                        .await?;

                    send_response(
                        &ctx,
                        &command,
                        command_config,
                        &title,
                        &format!("I am now listening to the emoji {}.", emoji),
                    )
                    .await
                }
                Action::Remove => {
                    // Check for the interaction response
                    let response = send_confirmation(
                        ctx,
                        command,
                        command_config,
                        "
                        Are you really sure you want to remove this emoji?
                        All saved reactions of this type will get lost.
                        This cannot be reversed!
                        ",
                        Duration::from_secs(config.general.interaction_timeout),
                    )
                    .await?;

                    match response {
                        Some(InteractionResponse::Continue) => {
                            // Delete entries
                            database
                                .client
                                .execute(
                                    "DELETE FROM reactions WHERE emoji = $1::INT",
                                    &[&emoji_id],
                                )
                                .await?;
                            database
                                .client
                                .execute(
                                    "DELETE FROM reaction_emojis WHERE emoji = $1::INT",
                                    &[&emoji_id],
                                )
                                .await?;

                            // TODO: Clean emoji table

                            send_response(
                                &ctx,
                                &command,
                                command_config,
                                &title,
                                &format!("I stopped listening to the emoji {}.", emoji),
                            )
                            .await
                        }
                        Some(InteractionResponse::Abort) => {
                            send_response(
                                ctx,
                                command,
                                command_config,
                                &title,
                                "Aborted the action.",
                            )
                            .await
                        }
                        None => Ok(()),
                    }
                }
            }
        }
        None => send_response(
            &ctx,
            &command,
            command_config,
            &title,
            "I couldn't find the specified emoji. Is it a valid emoji registered on this guild?",
        )
        .await,
    }
}
