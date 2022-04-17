use std::{
    fmt::{Display, Formatter},
    str::FromStr,
    time::Duration,
};

use serde_json::json;
use serenity::{
    builder::CreateActionRow,
    client::Context,
    http::Http,
    model::{
        channel::{ChannelType, ReactionType},
        guild::PartialGuild,
        id::{GuildId, RoleId},
        interactions::{
            application_command::ApplicationCommandInteraction, message_component::ButtonStyle,
        },
        invite::RichInvite,
        Permissions,
    },
};

use crate::{
    config::{Command, Config},
    data,
    database::client::Database,
    error::KowalskiError,
    error::KowalskiError::DiscordApiError,
    strings::ERR_CMD_ARGS_INVALID,
    utils::{
        parse_arg, send_confirmation, send_response, send_response_complex, InteractionResponse,
    },
};

enum Action {
    Create,
    List,
}

enum ComponentInteractionResponse {
    Left,
    Right,
    GetAdmin,
    RemoveAdmin,
    Ownership,
    Delete,
}

impl FromStr for Action {
    type Err = KowalskiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "create" => Ok(Action::Create),
            "list" => Ok(Action::List),
            _ => Err(DiscordApiError(ERR_CMD_ARGS_INVALID.to_string())),
        }
    }
}

impl Display for ComponentInteractionResponse {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            ComponentInteractionResponse::Left => "Go to left guild",
            ComponentInteractionResponse::Right => "Go to right guild",
            ComponentInteractionResponse::GetAdmin => "Give admin for guild",
            ComponentInteractionResponse::RemoveAdmin => "Remove admin for guild",
            ComponentInteractionResponse::Ownership => "Transfer ownership for guild",
            ComponentInteractionResponse::Delete => "Delete guild",
        };

        write!(f, "{}", name)
    }
}

impl FromStr for ComponentInteractionResponse {
    type Err = KowalskiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "left" => Ok(ComponentInteractionResponse::Left),
            "right" => Ok(ComponentInteractionResponse::Right),
            "get_admin" => Ok(ComponentInteractionResponse::GetAdmin),
            "remove_admin" => Ok(ComponentInteractionResponse::RemoveAdmin),
            "ownership" => Ok(ComponentInteractionResponse::Ownership),
            "delete" => Ok(ComponentInteractionResponse::Delete),
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

    let options = &command.data.options;

    // Parse argument
    let action = Action::from_str(parse_arg(options, 0)?)?;

    match action {
        Action::Create => {
            // Get count of owned guilds
            let count: i64 = database
                .client
                .query_one("SELECT COUNT(*) FROM guilds", &[])
                .await?
                .get(0);

            // Create guild
            let guild =
                create_guild(&ctx.http, &format!("Kowalski Guild #{}", count + 1), None).await?;
            // Add guild to database
            database
                .client
                .execute(
                    "
                    INSERT INTO guilds VALUES ($1::BIGINT)",
                    &[&(guild.id.0 as i64)],
                )
                .await?;
            // Get invite
            let invite = get_invite(ctx, &guild).await?;

            send_response(
                &ctx,
                &command,
                command_config,
                "Guild created",
                &format!(
                    "I have created a guild. You can join [here]({}).",
                    invite.url()
                ),
            )
            .await
        }
        Action::List => {
            // Get list of owned guilds
            let owned: Vec<GuildId> = database
                .client
                .query("SELECT guild FROM guilds", &[])
                .await?
                .iter()
                .map(|row| GuildId(row.get::<_, i64>(0) as u64))
                .collect();

            if owned.is_empty() {
                send_response(
                    ctx,
                    command,
                    command_config,
                    "Guild List",
                    "I currently don't own any guilds :(",
                )
                .await
            } else {
                // Current page index
                let mut index = 0;

                // Loop through interactions until there is a timeout
                while let Some(interaction) = show_guild(
                    ctx,
                    command,
                    command_config,
                    &owned,
                    index,
                    Duration::from_secs(config.general.interaction_timeout),
                )
                .await?
                {
                    match interaction {
                        ComponentInteractionResponse::Left => index -= 1,
                        ComponentInteractionResponse::Right => index += 1,
                        ComponentInteractionResponse::GetAdmin
                        | ComponentInteractionResponse::RemoveAdmin
                        | ComponentInteractionResponse::Ownership
                        | ComponentInteractionResponse::Delete => {
                            let current = owned.get(index).unwrap();
                            guild_action(
                                ctx,
                                command,
                                command_config,
                                &config,
                                &database,
                                current,
                                interaction,
                            )
                            .await?;

                            break;
                        }
                    }
                }

                // Remove components
                command
                    .edit_original_interaction_response(&ctx.http, |response| {
                        response.components(|components| components)
                    })
                    .await?;

                Ok(())
            }
        }
    }
}

async fn show_guild(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
    guilds: &Vec<GuildId>,
    index: usize,
    timeout: Duration,
) -> Result<Option<ComponentInteractionResponse>, KowalskiError> {
    // Get partial guild
    let partial_guild = guilds
        .get(index)
        .unwrap()
        .to_partial_guild(&ctx.http)
        .await?;

    // Create invite to the guild
    let invite = get_invite(ctx, &partial_guild).await?;

    // Create action rows
    let mut row1 = CreateActionRow::default();
    let mut row2 = CreateActionRow::default();
    row1.create_button(|button| {
        button
            .emoji(ReactionType::Unicode("⬅️".to_string()))
            .custom_id("left")
            .style(ButtonStyle::Secondary)
            .disabled(index == 0)
    })
    .create_button(|button| {
        button
            .label("Join Server")
            .url(invite.url())
            .style(ButtonStyle::Link)
    })
    .create_button(|button| {
        button
            .label("Delete Server")
            .custom_id("delete")
            .style(ButtonStyle::Secondary)
    })
    .create_button(|button| {
        button
            .emoji(ReactionType::Unicode("➡️".to_string()))
            .custom_id("right")
            .style(ButtonStyle::Secondary)
            .disabled(index >= guilds.len() - 1)
    });
    row2.create_button(|button| {
        button
            .label("Get Admin")
            .custom_id("get_admin")
            .style(ButtonStyle::Secondary)
    })
    .create_button(|button| {
        button
            .label("Remove Admin")
            .custom_id("remove_admin")
            .style(ButtonStyle::Secondary)
    })
    .create_button(|button| {
        button
            .label("Transfer Ownership")
            .custom_id("ownership")
            .style(ButtonStyle::Secondary)
    });

    let guild = ctx.cache.guild(partial_guild.clone()).await;

    // Send response
    send_response_complex(
        ctx,
        command,
        command_config,
        &format!("Guild '{}'", partial_guild.name),
        "",
        |embed| {
            let (members, boosters, tier, since) = {
                match &guild {
                    Some(guild) => (
                        guild.member_count.to_string(),
                        guild.premium_subscription_count.to_string(),
                        format!("Tier {}", guild.premium_tier.num()),
                        guild.joined_at.to_string(),
                    ),
                    None => (
                        "unknown".to_string(),
                        "unknown".to_string(),
                        "unknown".to_string(),
                        "unknown".to_string(),
                    ),
                }
            };

            embed.fields(vec![
                ("Members", members, true),
                ("Boosters", boosters, true),
                ("Premium level", tier, true),
                ("Member since", since, true),
            ])
        },
        vec![row1, row2],
    )
    .await?;

    // Get the message
    let message = command.get_interaction_response(&ctx.http).await?;
    // Get the interaction response
    let interaction = message
        .await_component_interaction(&ctx)
        .author_id(command.user.id.0)
        .timeout(timeout)
        .await;
    let response = match interaction {
        Some(interaction) => Some(ComponentInteractionResponse::from_str(
            interaction.data.custom_id.as_str(),
        )?),
        None => None,
    };

    Ok(response)
}

async fn guild_action(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
    config: &Config,
    database: &Database,
    current: &GuildId,
    interaction: ComponentInteractionResponse,
) -> Result<(), KowalskiError> {
    let mut guild = current.to_partial_guild(&ctx.http).await?;

    let title = format!("{} '{}'", interaction, guild.name);

    let content = match interaction {
        ComponentInteractionResponse::GetAdmin => {
            "Are you really sure you want to give yourself admin permissions on this guild?"
        }
        ComponentInteractionResponse::RemoveAdmin => {
            "Are you really sure you want to remove your admin permissions on this guild?"
        }
        ComponentInteractionResponse::Ownership => {
            "Are you really sure you want to transfer ownership of this guild?
                                    This cannot be reversed!"
        }
        ComponentInteractionResponse::Delete => {
            "Are you really sure you want to delete this guild?
                                    This cannot be reversed!"
        }
        _ => unreachable!(),
    };

    // Check for the interaction response
    let response = send_confirmation(
        ctx,
        command,
        command_config,
        content,
        Duration::from_secs(config.general.interaction_timeout),
    )
    .await?;

    match response {
        Some(InteractionResponse::Continue) => {
            match interaction {
                ComponentInteractionResponse::GetAdmin => {
                    // Get the member
                    let member = guild.member(&ctx.http, command.user.id).await;

                    match member {
                        Ok(mut member) => {
                            // Get an admin role
                            let admin = {
                                let role = guild
                                    .roles
                                    .iter()
                                    .filter(|(_, role)| role.permissions.administrator())
                                    .map(|(_, role)| role.clone())
                                    .next();

                                match role {
                                    Some(role) => role,
                                    None => {
                                        guild
                                            .create_role(&ctx.http, |role| {
                                                role.name("Kowalski Admin")
                                                    .permissions(Permissions::ADMINISTRATOR)
                                            })
                                            .await?
                                    }
                                }
                            };

                            // Give role to user
                            member.add_role(&ctx.http, admin.id).await?;

                            send_response(
                                ctx,
                                command,
                                command_config,
                                &title,
                                &format!("You are now an admin of guild '{}'.", guild.name),
                            )
                            .await
                        }
                        Err(_) => {
                            send_response(
                                ctx,
                                command,
                                command_config,
                                &title,
                                &format!(
                                    "I couldn't give you admin permissions. Are you currently user of the server?",
                                ),
                            )
                                .await
                        }
                    }
                }
                ComponentInteractionResponse::RemoveAdmin => {
                    // Get the member
                    let member = guild.member(&ctx.http, command.user.id).await;

                    match member {
                        Ok(mut member) => {
                            // Get admin roles of user
                            let roles: Vec<RoleId> = member
                                .roles(&ctx.cache)
                                .await
                                .unwrap_or_default()
                                .iter()
                                .filter(|role| role.permissions.administrator())
                                .map(|role| role.id)
                                .collect();
                            // Remove roles from member
                            member.remove_roles(&ctx.http, &roles).await?;

                            send_response(
                                ctx,
                                command,
                                command_config,
                                &title,
                                &format!("You are not an admin of guild '{}' anymore.", guild.name),
                            )
                            .await
                        }
                        Err(_) => {
                            send_response(
                                ctx,
                                command,
                                command_config,
                                &title,
                                &format!(
                                    "I couldn't remove your admin permissions. Are you currently user of the server?",
                                ),
                            )
                                .await
                        }
                    }
                }
                ComponentInteractionResponse::Ownership => {
                    // Transfer ownership
                    guild
                        .edit(&ctx.http, |guild| guild.owner(&command.user))
                        .await?;

                    if guild.owner_id == command.user.id {
                        // Remove guild from database
                        database
                            .client
                            .execute(
                                "DELETE FROM guilds WHERE guild = $1::BIGINT",
                                &[&(current.0 as i64)],
                            )
                            .await?;

                        send_response(
                            ctx,
                            command,
                            command_config,
                            &title,
                            &format!("You are now the owner of guild '{}'.", guild.name),
                        )
                        .await
                    } else {
                        send_response(
                            ctx,
                            command,
                            command_config,
                            &title,
                            &format!(
                                "I couldn't transfer ownership to you. Are you currently user of the server?",
                            ),
                        )
                            .await
                    }
                }
                ComponentInteractionResponse::Delete => {
                    // Delete guild (ignore result because of a library bug)
                    let _ = current.delete(&ctx.http).await;
                    // Remove guild from database
                    database
                        .client
                        .execute(
                            "DELETE FROM guilds WHERE guild = $1::BIGINT",
                            &[&(current.0 as i64)],
                        )
                        .await?;

                    send_response(
                        ctx,
                        command,
                        command_config,
                        &title,
                        &format!("I have removed guild '{}'.", guild.name),
                    )
                    .await
                }
                _ => unreachable!(),
            }
        }
        Some(InteractionResponse::Abort) => {
            send_response(ctx, command, command_config, &title, "Aborted the action.").await
        }
        None => Ok(()),
    }
}

async fn get_invite(ctx: &Context, guild: &PartialGuild) -> Result<RichInvite, KowalskiError> {
    // Get invite channel
    let channels = guild.channels(&ctx.http).await?;
    let invite_channel = channels
        .values()
        .filter(|channel| matches!(channel.kind, ChannelType::Text))
        .next();

    match invite_channel {
        Some(channel) => Ok(channel
            .create_invite(&ctx.http, |invite| invite.max_age(60))
            .await?),
        None => Err(DiscordApiError(
            "Couldn't find an invite channel".to_string(),
        )),
    }
}

// TODO: Replace with the library function as soon as it updated
async fn create_guild(
    http: impl AsRef<Http>,
    name: &str,
    icon: Option<&str>,
) -> serenity::Result<PartialGuild> {
    let map = json!({
        "icon": icon,
        "name": name,
    });

    http.as_ref().create_guild(&map).await
}
