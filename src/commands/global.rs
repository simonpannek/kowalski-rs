use itertools::Itertools;
use serenity::model::id::GuildId;
use serenity::{
    client::Context,
    model::{
        channel::ReactionType,
        id::EmojiId,
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
    pluralize,
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

    // Get user id
    let user_db_id = database.get_user(guild_id, user.id).await?;

    // Count active guilds of the user
    let guilds = {
        let row = database
            .client
            .query_one(
                "
        SELECT COUNT(*) guilds
        FROM users
        WHERE \"user\" = $1::BIGINT
        ",
                &[&user_db_id],
            )
            .await?;

        row.get::<_, Option<i64>>(0).unwrap_or_default()
    };

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
        WHERE user_to = $1::BIGINT
        ",
                &[&user_db_id],
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
        SELECT unicode, e.guild, guild_emoji, COUNT(*) FROM score_reactions r
        INNER JOIN emojis e ON r.emoji = e.id
        WHERE user_to = $1::BIGINT
        GROUP BY emoji, unicode, e.guild, guild_emoji
        ORDER BY count DESC
        ",
                &[&user_db_id],
            )
            .await?;

        let mut emojis = Vec::new();

        for row in rows {
            let unicode: Option<String> = row.get(0);
            let guild: Option<i64> = row.get(1);
            let guild_emoji: Option<i64> = row.get(2);
            let count: i64 = row.get(3);

            let emoji = match (unicode, guild, guild_emoji) {
                (Some(string), _, _) => ReactionType::Unicode(string),
                (_, Some(guild_db_id), Some(id)) => {
                    let guild_id = GuildId(guild_db_id as u64);
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
    // Get rank of the user
    let rank = {
        let row = database.client.query_opt("
            WITH ranks AS (
                SELECT user_to,
                RANK() OVER (
                    ORDER BY COUNT(*) FILTER (WHERE upvote) - COUNT(*) FILTER (WHERE NOT upvote) DESC, user_to
                ) rank
                FROM score_reactions r
                INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
                GROUP BY user_to
            )

            SELECT rank FROM ranks
            WHERE user_to = $1::BIGINT
            ", &[&user_db_id]).await?;

        row.map(|row| row.get::<_, i64>(0))
    };
    let rank = match rank {
        Some(rank) => rank.to_string(),
        None => String::from("not available"),
    };

    let (given_upvotes, given_downvotes) = {
        let row = database
            .client
            .query_one(
                "
        SELECT SUM(CASE WHEN upvote THEN 1 END) upvotes,
        SUM(CASE WHEN NOT upvote THEN 1 END) downvotes
        FROM score_reactions r
        INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
        WHERE user_from = $1::BIGINT
        ",
                &[&user_db_id],
            )
            .await?;

        let upvotes: Option<i64> = row.get(0);
        let downvotes: Option<i64> = row.get(1);

        (upvotes.unwrap_or_default(), downvotes.unwrap_or_default())
    };
    let given = given_upvotes - given_downvotes;
    let given_emojis = {
        let rows = database
            .client
            .query(
                "
        SELECT unicode, e.guild, guild_emoji, COUNT(*) FROM score_reactions r
        INNER JOIN emojis e ON r.emoji = e.id
        WHERE user_from = $1::BIGINT
        GROUP BY emoji, unicode, e.guild, guild_emoji
        ORDER BY count DESC
        ",
                &[&user_db_id],
            )
            .await?;

        let mut emojis = Vec::new();

        for row in rows {
            let unicode: Option<String> = row.get(0);
            let guild: Option<i64> = row.get(1);
            let guild_emoji: Option<i64> = row.get(2);
            let count: i64 = row.get(3);

            let emoji = match (unicode, guild, guild_emoji) {
                (Some(string), _, _) => ReactionType::Unicode(string),
                (_, Some(guild_db_id), Some(id)) => {
                    let guild_id = GuildId(guild_db_id as u64);
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
    let given_rank = {
        let row = database.client.query_opt("
            WITH ranks AS (
                SELECT user_from,
                RANK() OVER (
                    ORDER BY COUNT(*) FILTER (WHERE upvote) - COUNT(*) FILTER (WHERE NOT upvote) DESC, user_from
                ) rank
                FROM score_reactions r
                INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
                GROUP BY user_from
            )

            SELECT rank FROM ranks
            WHERE user_from = $1::BIGINT
            ", &[&user_db_id]).await?;

        row.map(|row| row.get::<_, i64>(0))
    };
    let given_rank = match given_rank {
        Some(given_rank) => given_rank.to_string(),
        None => String::from("not available"),
    };

    send_response_complex(
        &ctx,
        &command,
        command_config,
        &format!("Global stats of {}", user.name),
        &format!(
            "The user {} is currently active on {} shared with the bot.",
            user.mention(),
            pluralize!("guild", guilds)
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

            let mut given_emojis = given_emojis
                .iter()
                .map(|(reaction, count)| {
                    let f_count = *count as f64;
                    let f_total = (given_upvotes + given_downvotes) as f64;
                    format!(
                        "**{}x{}** ({:.1}%)",
                        count,
                        reaction.to_string(),
                        f_count / f_total * 100f64
                    )
                })
                .join(", ");
            if given_emojis.is_empty() {
                given_emojis = "Not available".to_string();
            }

            embed.fields(vec![
                (
                    "Score",
                    &format!(
                        "The user has a global score of **{}** [+{}, -{}] (rank **{}**).",
                        score, upvotes, downvotes, rank
                    ),
                    false,
                ),
                ("The following emojis were used", &emojis, false),
                (
                    "Given",
                    &format!(
                        "The user has given out a global score of **{}** [+{}, -{}] (rank **{}**).",
                        given, given_upvotes, given_downvotes, given_rank
                    ),
                    false,
                ),
                ("The following emojis were used", &given_emojis, false),
            ])
        },
        Vec::new(),
    )
    .await
}
