use std::str::FromStr;

use serenity::{
    client::Context,
    model::{
        channel::ReactionType,
        id::{ChannelId, EmojiId, MessageId, RoleId},
        interactions::application_command::ApplicationCommandInteraction,
    },
    prelude::Mentionable,
};

use crate::{
    config::{Command, Module},
    data,
    database::{client::Database, types::ModuleStatus},
    error::KowalskiError,
    utils::{parse_arg, send_response_complex},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get config and database
    let database = data!(ctx, Database);

    let options = &command.data.options;

    // Parse argument
    let module = Module::from_str(parse_arg(options, 0)?)?;

    // Get guild status
    let guild = command.guild_id.unwrap();
    let status = {
        let row = database
            .client
            .query_opt(
                "
                SELECT status
                FROM modules
                WHERE guild = $1::BIGINT
                ",
                &[&(guild.0 as i64)],
            )
            .await?;

        row.map_or(ModuleStatus::default(), |row| row.get(0))
    };

    // Get description of module
    let content = match module {
        Module::Owner => "The owner module includes all commands that can be executed by the owner. \
        If no bot owner is on the server or they should not be able to execute owner commands here, \
        this module should be disabled.",
        Module::Utility => "The utility module includes commands that provide commands not associated \
        with any other modules but may be useful for moderation. Utility commands are common commands, \
        often implemented by other bots as well. To avoid duplication, this module can be disabled when \
        required.",
        Module::Score => "The score module provides everything associated with the level-up system of \
        the bot. This includes commands for managing the level-up roles and commands to query the scores \
        and rankings of users. When the module is disabled, no reactions will get tracked as up- or \
        downvotes.",
        Module::ReactionRoles => "The reaction-roles module provides a reaction-role system. A reaction-\
        role binds an emoji and a message to a set of roles. When an user react to this message, the \
        bot will assign the defined set of roles to them. You can also limit a reaction-role. In this \
        case, the bot will only assign the reaction-role to users as long as there are slots available.",
        Module::Analyze => "The analyze module provides commands to analyze previous messages written \
        in a specific channel.",
    };

    let fields = {
        let mut fields = Vec::new();

        let enabled = match module {
            Module::Owner => status.owner,
            Module::Utility => status.utility,
            Module::Score => status.score,
            Module::ReactionRoles => status.reaction_roles,
            Module::Analyze => status.analyze,
        };

        if enabled {
            fields.push((
                "Module status".to_string(),
                format!(
                    "The module {:?} is currently enabled on this server.",
                    module
                ),
                false,
            ));

            match module {
                Module::Score => {
                    // Get up and downvotes
                    let (upvotes, downvotes) = {
                        let rows = database
                            .client
                            .query(
                                "
                                SELECT unicode, emoji_guild, upvote FROM score_emojis se
                                INNER JOIN emojis e ON se.emoji = e.id
                                WHERE guild = $1::BIGINT
                                ",
                                &[&(guild.0 as i64)],
                            )
                            .await?;

                        let mut upvotes = Vec::new();
                        let mut downvotes = Vec::new();

                        for row in rows {
                            let unicode: Option<String> = row.get(0);
                            let emoji_guild: Option<i64> = row.get(1);
                            let upvote: bool = row.get(2);

                            let emoji = match (unicode, emoji_guild) {
                                (Some(string), _) => ReactionType::Unicode(string),
                                (_, Some(id)) => {
                                    let emoji = guild.emoji(&ctx.http, EmojiId(id as u64)).await?;

                                    ReactionType::Custom {
                                        animated: emoji.animated,
                                        id: emoji.id,
                                        name: Some(emoji.name),
                                    }
                                }
                                _ => unreachable!(),
                            };

                            if upvote {
                                upvotes.push(emoji);
                            } else {
                                downvotes.push(emoji);
                            }
                        }

                        (upvotes, downvotes)
                    };

                    let mut upvotes = upvotes
                        .iter()
                        .map(|emoji| emoji.to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    if upvotes.is_empty() {
                        upvotes = "Not available".to_string();
                    }

                    let mut downvotes = downvotes
                        .iter()
                        .map(|emoji| emoji.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                        .to_string();
                    if downvotes.is_empty() {
                        downvotes = "Not available".to_string();
                    }

                    fields.push(("Upvotes".to_string(), upvotes, true));
                    fields.push(("Downvotes".to_string(), downvotes, true));

                    // Get roles
                    let roles: Vec<_> = {
                        let rows = database
                            .client
                            .query(
                                "
                                SELECT role, score FROM score_roles
                                WHERE guild = $1::BIGINT
                                ORDER BY score
                                ",
                                &[&(guild.0 as i64)],
                            )
                            .await?;

                        rows.iter()
                            .map(|row| (RoleId(row.get::<_, i64>(0) as u64), row.get::<_, i64>(1)))
                            .collect()
                    };

                    let mut roles = roles
                        .iter()
                        .map(|(role, score)| format!("{} **(>= {})**", role.mention(), score))
                        .collect::<Vec<_>>()
                        .join("\n");
                    if roles.is_empty() {
                        roles = "Not available".to_string();
                    }

                    fields.push(("Level-up roles".to_string(), roles, false));
                }
                Module::ReactionRoles => {
                    // Get roles
                    let roles = {
                        let rows = database
                            .client
                            .query(
                                "
                                SELECT channel, message, unicode, emoji_guild, role, slots
                                FROM reaction_roles rr
                                INNER JOIN emojis e ON emoji = id
                                WHERE guild = $1::BIGINT
                                ",
                                &[&(guild.0 as i64)],
                            )
                            .await?;

                        let mut roles = Vec::new();

                        for row in rows {
                            let link = {
                                let channel_id = ChannelId(row.get::<_, i64>(0) as u64);
                                let message_id = MessageId(row.get::<_, i64>(1) as u64);
                                let message = channel_id.message(&ctx.http, message_id).await?;

                                message.link()
                            };
                            let unicode: Option<String> = row.get(2);
                            let emoji_guild: Option<i64> = row.get(3);
                            let emoji = match (unicode, emoji_guild) {
                                (Some(string), _) => ReactionType::Unicode(string),
                                (_, Some(id)) => {
                                    let emoji = guild.emoji(&ctx.http, EmojiId(id as u64)).await?;

                                    ReactionType::Custom {
                                        animated: emoji.animated,
                                        id: emoji.id,
                                        name: Some(emoji.name),
                                    }
                                }
                                _ => unreachable!(),
                            };
                            let role = RoleId(row.get::<_, i64>(4) as u64);
                            let slots: Option<i32> = row.get(5);

                            roles.push((link, emoji, role, slots));
                        }

                        roles
                    };

                    let mut roles = roles
                        .iter()
                        .map(|(link, emoji, role, slots)| {
                            let mut content = format!(
                                "{} when reacting with {} [here]({})",
                                role.mention(),
                                emoji.to_string(),
                                link
                            );

                            if let Some(slots) = slots {
                                content.push_str(&format!(
                                    " (There are currently {} slots available)",
                                    slots
                                ));
                            }

                            content
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if roles.is_empty() {
                        roles = "Not available".to_string();
                    }

                    // TODO: Add link to message
                    fields.push(("Reaction-roles".to_string(), roles, false));
                }
                _ => {}
            }
        } else {
            fields.push((
                "Module status".to_string(),
                format!(
                    "The module {:?} is currently disabled on this server.
                    I won't display any additional information.",
                    module
                ),
                false,
            ));
        }

        fields
    };

    send_response_complex(
        &ctx,
        &command,
        command_config,
        &format!("Information about module '{:?}'", module),
        content,
        |embed| embed.fields(fields.clone()),
        Vec::new(),
    )
    .await
}
