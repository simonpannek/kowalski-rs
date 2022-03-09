use serenity::{
    client::Context,
    model::interactions::application_command::{
        ApplicationCommandInteraction, ApplicationCommandInteractionDataOptionValue::User,
    },
    prelude::Mentionable,
};

use crate::{
    config::Command,
    database::client::Database,
    error::ExecutionError,
    strings::{ERR_API_LOAD, ERR_DATA_ACCESS},
    utils::{parse_arg_resolved, send_response},
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
    let user = if !options.is_empty() {
        match parse_arg_resolved(options, 0)? {
            User(user, ..) => Ok(user),
            _ => Err(ExecutionError::new(ERR_API_LOAD)),
        }?
    } else {
        &command.user
    };

    // Get guild
    let guild = command.guild_id.ok_or(ExecutionError::new(ERR_API_LOAD))?;

    // Get rank of the user
    let rank = {
        let row = database.client.query_opt("
            SELECT rank FROM (
                SELECT
                    user_to,
                    RANK() OVER (
                        ORDER BY COUNT(*) FILTER (WHERE upvote) - COUNT(*) FILTER (WHERE NOT upvote) DESC, user_to
                    ) rank
                FROM reactions r
                INNER JOIN score_emojis re ON r.emoji = re.emoji
                WHERE r.guild = $1::BIGINT
                GROUP BY user_to
            ) AS ranks
            WHERE user_to = $2::BIGINT
            ", &[&i64::from(guild), &i64::from(user.id)]).await?;

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
