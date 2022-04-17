use std::{
    fmt::{Display, Formatter},
    str::FromStr,
    time::Duration,
};

use serenity::{
    client::Context,
    collector::ReactionAction,
    model::{
        channel::ReactionType,
        interactions::application_command::{
            ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue::Role,
        },
    },
    prelude::Mentionable,
};

use crate::{
    config::Command,
    config::Config,
    database::client::Database,
    error::ExecutionError,
    strings::{ERR_API_LOAD, ERR_CMD_ARGS_INVALID, ERR_DATA_ACCESS},
    utils::{parse_arg, parse_arg_resolved, send_response},
};

enum Action {
    Add,
    Remove,
}

impl Display for Action {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Action::Add => "Add",
            Action::Remove => "Remove",
        };

        write!(f, "{}", name)
    }
}

impl FromStr for Action {
    type Err = ExecutionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "add" => Ok(Action::Add),
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
    let role = match parse_arg_resolved(options, 1)? {
        Role(role) => Ok(role),
        _ => Err(ExecutionError::new(ERR_API_LOAD)),
    }?;
    let slots = {
        if options.len() > 2 {
            Some(parse_arg::<i64>(options, 2)?)
        } else {
            None
        }
    };

    let title = format!("{} reaction-role for {}", action, role.name);

    send_response(
        &ctx,
        &command,
        command_config,
        &title,
        &format!("React to the message to which you want to {} the reaction-role with the designated emoji.", action),
    )
        .await?;

    // Wait for the reaction
    let reaction = guild
        .await_reaction(&ctx)
        .guild_id(guild)
        .author_id(command.user.id)
        .removed(false)
        .timeout(Duration::from_secs(config.general.interaction_timeout))
        .await;

    match reaction.as_ref() {
        Some(reaction) => {
            match reaction.as_ref() {
                ReactionAction::Added(reaction) => {
                    // Check whether the emoji is available on the guild
                    if let ReactionType::Custom { id, .. } = &reaction.emoji {
                        if let Err(_) = guild.emoji(&ctx.http, *id).await {
                            return send_response(
                                ctx,
                                command,
                                command_config,
                                &title,
                                "I couldn't find the specified emoji. Is it a valid emoji registered on this guild?"
                            ).await;
                        }
                    }

                    // Get the id of the emoji in the emoji table
                    let emoji = database.get_emoji(&reaction.emoji).await?;

                    // Convert the ids to integers
                    let guild_id = guild.0 as i64;
                    let channel_id = command.channel_id.0 as i64;
                    let message_id = reaction.message_id.0 as i64;
                    let role_id = role.id.0 as i64;

                    match action {
                        Action::Add => {
                            // Insert into the database if there is no entry yet
                            database
                                .client
                                .execute(
                                    "
                            WITH duplicate AS (
                                SELECT * FROM reaction_roles
                                WHERE guild = $1::BIGINT AND channel = $2::BIGINT
                                AND message = $3::BIGINT AND emoji = $4::INT
                                AND role = $5::BIGINT
                            )

                            INSERT INTO reaction_roles
                            SELECT $1::BIGINT, $2::BIGINT, $3::BIGINT, $4::INT, $5::BIGINT
                            WHERE NOT EXISTS /duplicate
                            ",
                                    &[&guild_id, &channel_id, &message_id, &emoji, &role_id],
                                )
                                .await?;

                            // Update the slots
                            match slots {
                                Some(slots) => {
                                    database
                                        .client
                                        .execute(
                                            "
                                    UPDATE reaction_roles
                                    SET slots = $5::BIGINT
                                    WHERE guild = $1::BIGINT AND channel = $2::BIGINT
                                    AND message = $3::BIGINT AND emoji = $4::INT
                                    AND role = $5::BIGINT
                                    ",
                                            &[
                                                &guild_id,
                                                &channel_id,
                                                &message_id,
                                                &emoji,
                                                &role_id,
                                                &slots,
                                            ],
                                        )
                                        .await?;
                                }
                                None => {
                                    database
                                        .client
                                        .execute(
                                            "
                                    UPDATE reaction_roles
                                    SET slots = NULL
                                    WHERE guild = $1::BIGINT AND channel = $2::BIGINT
                                    AND message = $3::BIGINT AND emoji = $4::INT
                                    AND role = $5::BIGINT
                                    ",
                                            &[
                                                &guild_id,
                                                &channel_id,
                                                &message_id,
                                                &emoji,
                                                &role_id,
                                            ],
                                        )
                                        .await?;
                                }
                            }

                            // React to the message
                            let message = reaction.message(&ctx.http).await?;
                            message.react(&ctx.http, reaction.emoji.clone()).await?;
                            // Remove the reaction of the user
                            reaction.delete(&ctx.http).await?;

                            let content = format!(
                                "I will assign the role {} to users which react with {} [here]({}).
                                There are {} role-slots available.",
                                role.mention(),
                                &reaction.emoji.to_string(),
                                &message.link(),
                                slots.map_or("unlimited".to_string(), |num| num.to_string())
                            );

                            send_response(ctx, command, command_config, &title, &content).await
                        }
                        Action::Remove => {
                            database
                                .client
                                .execute(
                                    "
                            DELETE FROM reaction_roles
                            WHERE guild = $1::BIGINT AND channel = $2::BIGINT
                            AND message = $3::BIGINT AND emoji = $4::INT AND role = $5::BIGINT
                            ",
                                    &[&guild_id, &channel_id, &message_id, &emoji, &role_id],
                                )
                                .await?;

                            // Remove the reactions of the message
                            let message = reaction.message(&ctx.http).await?;
                            message
                                .delete_reaction_emoji(&ctx.http, reaction.emoji.clone())
                                .await?;

                            let content = format!(
                                "I will no longer assign the role {} to users which react with {} [here]({}).",
                                role.mention(),
                                &reaction.emoji.to_string(),
                                &message
                                    .link()
                            );

                            send_response(ctx, command, command_config, &title, &content).await
                        }
                    }
                }
                ReactionAction::Removed(_) => unreachable!(),
            }
        }
        None => {
            send_response(
                ctx,
                command,
                command_config,
                "Timed out",
                "You took too long to respond :(",
            )
            .await
        }
    }
}
