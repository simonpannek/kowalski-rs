use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
    model::interactions::application_command::ApplicationCommandInteractionDataOptionValue::Role,
    prelude::Mentionable,
};

use crate::{
    config::Command,
    database::client::Database,
    error::ExecutionError,
    strings::{ERR_API_LOAD, ERR_DATA_ACCESS},
    utils::{parse_arg, parse_arg_resolved, send_response},
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

    // Parse first argument
    let role = match parse_arg_resolved(options, 0)? {
        Role(role) => Ok(role),
        _ => Err(ExecutionError::new(ERR_API_LOAD)),
    }?;

    // Get guild and role ids
    let guild_id = i64::from(role.guild_id);
    let role_id = i64::from(role.id);

    let title = format!("Set cooldown for {}", role.name);

    if options.len() > 1 {
        // Parse second argument
        let cooldown: i64 = parse_arg(options, 1)?;

        // Insert or update entry
        database
            .client
            .execute(
                "
        INSERT INTO score_cooldowns VALUES ($1::BIGINT, $2::BIGINT, $3::BIGINT)
        ON CONFLICT (guild, role) DO UPDATE SET cooldown = $3::BIGINT
        ",
                &[&guild_id, &role_id, &cooldown],
            )
            .await?;

        send_response(
            &ctx,
            &command,
            command_config,
            &title,
            &format!(
                "The role {} now has a reaction-cooldown of {} seconds.",
                role.mention(),
                cooldown
            ),
        )
        .await
    } else {
        // Delete cooldown
        database
            .client
            .execute(
                "
        DELETE FROM score_cooldowns
        WHERE guild = $1::BIGINT AND role = $2::BIGINT
        ",
                &[&guild_id, &role_id],
            )
            .await?;

        send_response(
            &ctx,
            &command,
            command_config,
            &title,
            &format!(
                "The role {} now has the default reaction-cooldown.",
                role.mention()
            ),
        )
        .await
    }
}
