use itertools::Itertools;
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
    config::Command, data, database::client::Database, error::KowalskiError, pluralize,
    utils::send_response,
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

    // Get reaction roles
    let roles = {
        let rows = database
            .client
            .query(
                "
                SELECT channel, message, unicode, guild_emoji, role, slots
                FROM reaction_roles rr
                INNER JOIN emojis e ON emoji = id
                WHERE rr.guild = $1::BIGINT
                ORDER BY channel, message
                ",
                &[&guild_db_id],
            )
            .await?;

        let mut roles = Vec::new();

        for row in rows {
            let channel_id = ChannelId(row.get::<_, i64>(0) as u64);
            let message_id = MessageId(row.get::<_, i64>(1) as u64);
            let unicode: Option<String> = row.get(2);
            let guild_emoji: Option<i64> = row.get(3);
            let emoji = match (unicode, guild_emoji) {
                (Some(string), _) => ReactionType::Unicode(string),
                (_, Some(id)) => {
                    let emoji = guild_id.emoji(&ctx.http, EmojiId(id as u64)).await?;

                    ReactionType::Custom {
                        animated: emoji.animated,
                        id: emoji.id,
                        name: Some(emoji.name),
                    }
                }
                _ => unreachable!(),
            };
            let role_id = RoleId(row.get::<_, i64>(4) as u64);
            let slots: Option<i32> = row.get(5);

            roles.push((channel_id, message_id, emoji, role_id, slots));
        }

        roles
    };

    let roles = roles
        .iter()
        .map(|(channel_id, message_id, emoji, role_id, slots)| {
            let mut content = format!(
                "{} when reacting with {} [here]({}).",
                role_id.mention(),
                emoji.to_string(),
                message_id.link(*channel_id, Some(guild_id))
            );

            if let Some(slots) = slots {
                content.push_str(&format!(
                    " (There {} currently {} available)",
                    if *slots == 1 { "is" } else { "are" },
                    pluralize!("slot", *slots)
                ));
            }

            content
        })
        .join("\n");

    let title = "Reaction roles";

    if roles.is_empty() {
        send_response(
            ctx,
            command,
            command_config,
            title,
            "There are no reaction roles registered on this guild.",
        )
        .await
    } else {
        send_response(
            ctx,
            command,
            command_config,
            title,
            &format!(
                "The following reaction roles are not registered on this guild:\n\n{}",
                roles
            ),
        )
        .await
    }
}
