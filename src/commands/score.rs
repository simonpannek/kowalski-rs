use itertools::Itertools;
use serenity::{
    client::Context,
    model::{
        channel::ReactionType,
        id::{EmojiId, UserId},
        interactions::application_command::{
            ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue::User,
        },
    },
    prelude::Mentionable,
};

use crate::{
    config::Command,
    data,
    database::client::Database,
    error::KowalskiError,
    utils::{parse_arg_resolved, send_response_complex},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    // Get database
    let database = data!(ctx, Database);

    let options = &command.data.options;

    // Parse argument (use command user as fallback)
    let user = if !options.is_empty() {
        match parse_arg_resolved(options, 0)? {
            User(user, ..) => user,
            _ => unreachable!(),
        }
    } else {
        &command.user
    };

    // Get guild
    let guild_id = command.guild_id.unwrap();

    // Get guild and user ids
    let guild_db_id = database.get_guild(guild_id).await?;
    let user_db_id = database.get_user(guild_id, user.id).await?;

    // Analyze reactions of the user
    let (upvotes, downvotes) = {
        let row = database
            .client
            .query_one(
                "
        SELECT SUM(CASE WHEN upvote THEN 1 END) upvotes,
        SUM(CASE WHEN NOT upvote THEN 1 END) downvotes
        FROM score_reactions r
        INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
        WHERE r.guild = $1::BIGINT AND user_to = $2::BIGINT
        ",
                &[&guild_db_id, &user_db_id],
            )
            .await?;

        let upvotes: Option<i64> = row.get(0);
        let downvotes: Option<i64> = row.get(1);

        (upvotes.unwrap_or_default(), downvotes.unwrap_or_default())
    };
    let score = upvotes - downvotes;
    let emojis = {
        let rows = database
            .client
            .query(
                "
        SELECT unicode, guild_emoji, COUNT(*) FROM score_reactions r
        INNER JOIN emojis e ON r.emoji = e.id
        WHERE r.guild = $1::BIGINT AND user_to = $2::BIGINT
        GROUP BY emoji, unicode, guild_emoji
        ORDER BY count DESC
        ",
                &[&guild_db_id, &user_db_id],
            )
            .await?;

        let mut emojis = Vec::new();

        for row in rows {
            let unicode: Option<String> = row.get(0);
            let guild_emoji: Option<i64> = row.get(1);
            let count: i64 = row.get(2);

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

            emojis.push((emoji, count));
        }

        emojis
    };
    let rank = {
        let row = database.client.query_opt("
            WITH ranks AS (
                SELECT user_to,
                RANK() OVER (
                    ORDER BY COUNT(*) FILTER (WHERE upvote) - COUNT(*) FILTER (WHERE NOT upvote) DESC, user_to
                ) rank
                FROM score_reactions r
                INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
                WHERE r.guild = $1::BIGINT
                GROUP BY user_to
            )

            SELECT rank FROM ranks
            WHERE user_to = $2::BIGINT
            ", &[&guild_db_id, &user_db_id]).await?;

        row.map(|row| row.get::<_, i64>(0))
    };
    let rank = match rank {
        Some(rank) => rank.to_string(),
        None => String::from("not available"),
    };

    let users: Vec<_> = {
        let rows = database
            .client
            .query(
                "
        SELECT user_from, COUNT(*) FILTER (WHERE upvote) upvotes,
        COUNT(*) FILTER (WHERE NOT upvote) downvotes
        FROM score_reactions r
        INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
        WHERE r.guild = $1::BIGINT AND user_to = $2::BIGINT AND native = true
        GROUP BY user_from
        ORDER BY COUNT(*) FILTER (WHERE upvote) - COUNT(*) FILTER (WHERE NOT upvote) DESC
        LIMIT 5
        ",
                &[&guild_db_id, &user_db_id],
            )
            .await?;

        rows.iter()
            .map(|row| {
                let user: i64 = row.get(0);
                let upvotes: Option<i64> = row.get(1);
                let downvotes: Option<i64> = row.get(2);

                (
                    UserId(user as u64),
                    upvotes.unwrap_or_default(),
                    downvotes.unwrap_or_default(),
                )
            })
            .collect()
    };

    send_response_complex(
        &ctx,
        &command,
        command_config,
        &format!("Score of {}", user.name),
        &format!(
            "The user {} currently has a score of **{}** [+{}, -{}] (rank **{}**).",
            user.mention(),
            score,
            upvotes,
            downvotes,
            rank
        ),
        |embed| {
            let mut emojis = emojis
                .iter()
                .map(|(reaction, count)| {
                    let f_count = *count as f64;
                    let f_total = (upvotes + downvotes) as f64;
                    format!(
                        "**{}x{}** ({:.1}%)",
                        count,
                        reaction.to_string(),
                        f_count / f_total * 100f64
                    )
                })
                .join(", ");
            if emojis.is_empty() {
                emojis = "Not available".to_string();
            }

            let mut users = users
                .iter()
                .map(|(user, upvotes, downvotes)| {
                    format!(
                        "{}: **{}** [+{}, -{}]",
                        user.mention(),
                        upvotes - downvotes,
                        upvotes,
                        downvotes
                    )
                })
                .join("\n");
            if users.is_empty() {
                users = "Not available".to_string();
            }

            embed.fields(vec![
                ("Emojis", emojis, false),
                ("Top 5 benefactors", users, false),
            ])
        },
        Vec::new(),
    )
    .await
}
