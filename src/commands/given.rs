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

    let guild_id = command.guild_id.unwrap();

    // Get guild id
    let guild_db_id = database.get_guild(guild_id).await?;
    let user_db_id = database.get_user(guild_id, user.id).await?;

    // Analyze reactions from the user
    let (upvotes, downvotes) = {
        let row = database
            .client
            .query_one(
                "
        SELECT SUM(CASE WHEN upvote THEN 1 END) upvotes,
        SUM(CASE WHEN NOT upvote THEN 1 END) downvotes
        FROM score_reactions r
        INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
        WHERE r.guild = $1::BIGINT AND user_from = $2::BIGINT
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
        WHERE r.guild = $1::BIGINT AND user_from = $2::BIGINT
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
                SELECT user_from,
                RANK() OVER (
                    ORDER BY COUNT(*) FILTER (WHERE upvote) - COUNT(*) FILTER (WHERE NOT upvote) DESC, user_from
                ) rank
                FROM score_reactions r
                INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
                WHERE r.guild = $1::BIGINT
                GROUP BY user_from
            )

            SELECT rank FROM ranks
            WHERE user_from = $2::BIGINT
            ", &[&guild_db_id, &user_db_id]).await?;

        row.map(|row| row.get::<_, i64>(0))
    };
    let rank = match rank {
        Some(rank) => rank.to_string(),
        None => String::from("not available"),
    };

    let top_users: Vec<_> = {
        let rows = database
            .client
            .query(
                "
        SELECT user_to, COUNT(*) FILTER (WHERE upvote) upvotes,
        COUNT(*) FILTER (WHERE NOT upvote) downvotes,
        SUM(CASE WHEN upvote THEN 1 ELSE -1 END) FILTER (WHERE NOT native) transferred
        FROM score_reactions r
        INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
        WHERE r.guild = $1::BIGINT AND user_from = $2::BIGINT
        GROUP BY user_to
        HAVING COUNT(*) FILTER (WHERE upvote) - COUNT(*) FILTER (WHERE NOT upvote) >= 0
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
                let transferred: Option<i64> = row.get(3);

                (
                    UserId(user as u64),
                    upvotes.unwrap_or_default(),
                    downvotes.unwrap_or_default(),
                    transferred.unwrap_or_default(),
                )
            })
            .collect()
    };

    let bottom_users: Vec<_> = {
        let rows = database
            .client
            .query(
                "
        SELECT user_to, COUNT(*) FILTER (WHERE upvote) upvotes,
        COUNT(*) FILTER (WHERE NOT upvote) downvotes,
        SUM(CASE WHEN upvote THEN 1 ELSE -1 END) FILTER (WHERE NOT native) transferred
        FROM score_reactions r
        INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
        WHERE r.guild = $1::BIGINT AND user_from = $2::BIGINT
        GROUP BY user_to
        HAVING COUNT(*) FILTER (WHERE upvote) - COUNT(*) FILTER (WHERE NOT upvote) < 0
        ORDER BY COUNT(*) FILTER (WHERE upvote) - COUNT(*) FILTER (WHERE NOT upvote) ASC
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
                let transferred: Option<i64> = row.get(3);

                (
                    UserId(user as u64),
                    upvotes.unwrap_or_default(),
                    downvotes.unwrap_or_default(),
                    transferred.unwrap_or_default(),
                )
            })
            .collect()
    };

    send_response_complex(
        &ctx,
        &command,
        command_config,
        &format!("Score given out by {}", user.name),
        &format!(
            "The user {} has given out a total score of **{}** [+{}, -{}] (rank **{}**).",
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

            let mut top_users = top_users
                .iter()
                .map(|(user, upvotes, downvotes, transferred)| {
                    format!(
                        "{}: **{}** [+{}, -{}] ({} transferred)",
                        user.mention(),
                        upvotes - downvotes,
                        upvotes,
                        downvotes,
                        transferred
                    )
                })
                .join("\n");
            if top_users.is_empty() {
                top_users = "Not available".to_string();
            }

            let mut bottom_users = bottom_users
                .iter()
                .map(|(user, upvotes, downvotes, transferred)| {
                    format!(
                        "{}: **{}** [+{}, -{}] ({} transferred)",
                        user.mention(),
                        upvotes - downvotes,
                        upvotes,
                        downvotes,
                        transferred
                    )
                })
                .join("\n");
            if bottom_users.is_empty() {
                bottom_users = "Not available".to_string();
            }

            embed.fields(vec![
                ("Favorite emojis", emojis, false),
                ("Top 5 upvoted", top_users, false),
                ("Top 5 downvoted", bottom_users, false),
            ])
        },
        Vec::new(),
    )
    .await
}
