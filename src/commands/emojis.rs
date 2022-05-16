use itertools::Itertools;
use serenity::{
    client::Context,
    model::{
        channel::ReactionType, id::EmojiId,
        interactions::application_command::ApplicationCommandInteraction,
    },
};

use crate::{
    config::Command, data, database::client::Database, error::KowalskiError, utils::send_response,
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

    // Get up- and downvote emojis
    let (upvotes, downvotes) = {
        let rows = database
            .client
            .query(
                "
                SELECT unicode, emoji_guild, upvote FROM score_emojis se
                INNER JOIN emojis e ON se.emoji = e.id
                WHERE guild = $1::BIGINT
                ",
                &[&guild_db_id],
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
                    let emoji = guild_id.emoji(&ctx.http, EmojiId(id as u64)).await?;

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

    let title = "Reaction emojis";

    if upvotes.is_empty() && downvotes.is_empty() {
        send_response(
            ctx,
            command,
            command_config,
            title,
            "There are no reaction emojis registered on this guild.",
        )
        .await
    } else {
        let mut content =
            "The following reaction emojis are registered on this guild:\n\n".to_string();

        if !upvotes.is_empty() {
            content.push_str(&format!(
                "**Upvotes:** {}\n",
                upvotes.iter().map(|emoji| emoji.to_string()).join(", ")
            ));
        }

        if !downvotes.is_empty() {
            content.push_str(&format!(
                "**Downvotes:** {}\n",
                downvotes.iter().map(|emoji| emoji.to_string()).join(", ")
            ));
        }

        send_response(ctx, command, command_config, title, &content).await
    }
}
