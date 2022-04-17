use serenity::{
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue::User,
    },
    prelude::Mentionable,
};

use crate::{
    config::Command,
    data,
    database::client::Database,
    error::KowalskiError,
    utils::{parse_arg_resolved, send_response},
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
    let guild = command.guild_id.unwrap();

    // Get rank of the user
    let rank = {
        let row = database.client.query_opt("
            WITH ranks AS (
                SELECT user_to,
                RANK() OVER (
                    ORDER BY COUNT(*) FILTER (WHERE upvote) - COUNT(*) FILTER (WHERE NOT upvote) DESC, user_to
                ) rank
                FROM reactions r
                INNER JOIN score_emojis se ON r.guild = se.guild AND r.emoji = se.emoji
                WHERE r.guild = $1::BIGINT
                GROUP BY user_to
            )

            SELECT rank FROM ranks
            WHERE user_to = $2::BIGINT
            ", &[&(guild.0 as i64), &(user.id.0 as i64)]).await?;

        row.map(|row| row.get::<_, i64>(0))
    };

    let content = match rank {
        Some(rank) => {
            format!(
                "The user {} is currently ranked **number {}**.",
                user.mention(),
                rank
            )
        }
        None => {
            format!("The user {} does not have a rank yet.", user.mention())
        }
    };

    send_response(
        &ctx,
        &command,
        command_config,
        &format!("Score of {}", user.name),
        &content,
    )
    .await
}
