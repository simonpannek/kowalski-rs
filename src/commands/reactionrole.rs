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
    data,
    database::client::Database,
    error::KowalskiError,
    error::KowalskiError::DiscordApiError,
    strings::ERR_CMD_ARGS_INVALID,
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
    type Err = KowalskiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "add" => Ok(Action::Add),
            "remove" => Ok(Action::Remove),
            _ => Err(DiscordApiError(ERR_CMD_ARGS_INVALID.to_string())),
        }
    }
}

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get config and database
    let (config, database) = data!(ctx, (Config, Database));

    let guild_id = command.guild_id.unwrap();

    let options = &command.data.options;

    // Parse arguments
    let action = Action::from_str(parse_arg(options, 0)?)?;
    let role = match parse_arg_resolved(options, 1)? {
        Role(role) => role,
        _ => unreachable!(),
    };
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
    let reaction = guild_id
        .await_reaction(&ctx)
        .guild_id(guild_id)
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
                        if let Err(_) = guild_id.emoji(&ctx.http, *id).await {
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
                    let emoji = database.get_emoji(guild_id, &reaction.emoji).await?;

                    // Get the guild, role, channel and message ids
                    let guild_db_id = database.get_guild(guild_id).await?;
                    let role_db_id = database.get_role(guild_id, role.id).await?;
                    let channel_db_id = database.get_channel(guild_id, reaction.channel_id).await?;
                    let message_db_id = database
                        .get_message(guild_id, reaction.channel_id, reaction.message_id)
                        .await?;

                    match action {
                        Action::Add => {
                            // Insert into the database if there is no entry yet
                            database
                                .client
                                .execute(
                                    "
                                INSERT INTO reaction_roles
                                VALUES ($1::BIGINT, $2::BIGINT, $3::BIGINT, $4::INT, $5::BIGINT,
                                    $6::BIGINT)
                                ON CONFLICT (guild, channel, message, emoji, role)
                                DO UPDATE SET slots = $6::BIGINT
                                ",
                                    &[
                                        &guild_db_id,
                                        &channel_db_id,
                                        &message_db_id,
                                        &emoji,
                                        &role_db_id,
                                        &slots,
                                    ],
                                )
                                .await?;

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
                                    &[
                                        &guild_db_id,
                                        &channel_db_id,
                                        &message_db_id,
                                        &emoji,
                                        &role_db_id,
                                    ],
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
