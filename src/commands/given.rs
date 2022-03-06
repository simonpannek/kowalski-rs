use serenity::{
    client::Context,
    model::{
        channel::ReactionType,
        id::{EmojiId, UserId},
        interactions::application_command::ApplicationCommandInteraction,
    },
    prelude::Mentionable,
};

use crate::{
    config::Command,
    database::client::Database,
    error::ExecutionError,
    strings::{ERR_API_LOAD, ERR_DATA_ACCESS},
    utils::{parse_arg, send_response_complex},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), ExecutionError> {
    // Get database
    let database = {
        let data = ctx.data.read().await;

        data.get::<Database>().expect(ERR_DATA_ACCESS).clone()
    };

    let options = &command.data.options;

    // Parse argument (use command user as fallback)
    let user_id = if !options.is_empty() {
        let user = parse_arg::<String>(options, 0)?
            .parse()
            .map_err(|why| ExecutionError::new(&format!("{}: {}", ERR_API_LOAD, why)))?;
        UserId(user)
    } else {
        command.user.id
    };

    // Get guild and user
    let guild = command.guild_id.ok_or(ExecutionError::new(ERR_API_LOAD))?;
    let user = user_id.to_user(&ctx.http).await?;

    // Analyze reactions from the user
    let (upvotes, downvotes) = {
        let row = database
            .client
            .query_one(
                "
        SELECT
            SUM(CASE WHEN upvote THEN 1 END) upvotes,
            SUM(CASE WHEN NOT upvote THEN 1 END) downvotes
        FROM reactions r
        INNER JOIN score_emojis re ON r.guild = re.guild AND r.emoji = re.emoji
        WHERE r.guild = $1::BIGINT AND user_from = $2::BIGINT
        ",
                &[&i64::from(guild), &i64::from(user_id)],
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
        SELECT unicode, emoji_guild, COUNT(*) FROM reactions r
        INNER JOIN emojis e ON r.emoji = e.id
        WHERE guild = $1::BIGINT AND user_from = $2::BIGINT
        GROUP BY emoji, unicode, emoji_guild
        ORDER BY count DESC
        ",
                &[&i64::from(guild), &i64::from(user_id)],
            )
            .await?;

        let mut emojis = Vec::new();

        for row in rows {
            let unicode: Option<String> = row.get(0);
            let emoji_guild: Option<i64> = row.get(1);
            let count: i64 = row.get(2);

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

            emojis.push((emoji, count));
        }

        emojis
    };
    let users: Vec<_> = {
        let rows = database
            .client
            .query(
                "
        SELECT
            user_to,
            COUNT(*) FILTER (WHERE upvote) upvotes,
            COUNT(*) FILTER (WHERE NOT upvote) downvotes
        FROM reactions r
        INNER JOIN score_emojis re ON r.guild = re.guild AND r.emoji = re.emoji
        WHERE r.guild = $1::BIGINT AND user_from = $2::BIGINT
        GROUP BY user_to
        ORDER BY COUNT(*) FILTER (WHERE upvote) - COUNT(*) FILTER (WHERE NOT upvote) DESC
        LIMIT 5
        ",
                &[&i64::from(guild), &i64::from(user_id)],
            )
            .await?;

        rows.iter()
            .map(|row| {
                let user: i64 = row.get(0);
                let upvotes: i64 = row.get(1);
                let downvotes: i64 = row.get(2);

                (UserId::from(user as u64), upvotes, downvotes)
            })
            .collect()
    };

    send_response_complex(
        &ctx,
        &command,
        command_config,
        &format!("Score given out by {}", user.name),
        &format!(
            "The user {} has given out a total score of **{}** [+{}, -{}].",
            user_id.mention(),
            score,
            upvotes,
            downvotes
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
                .collect::<Vec<_>>()
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
                .collect::<Vec<_>>()
                .join("\n");
            if users.is_empty() {
                users = "Not available".to_string();
            }

            embed.fields(vec![
                ("Favorite emojis", emojis, false),
                ("Top 5 given to", users, false),
            ])
        },
        Vec::new(),
    )
    .await
}
