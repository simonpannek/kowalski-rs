use chrono::{Duration, DurationRound, Utc};
use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{
    config::Command,
    config::Config,
    data,
    database::client::Database,
    error::KowalskiError,
    history::History,
    pluralize,
    utils::{parse_arg, parse_arg_name, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), KowalskiError> {
    let (config, database, history_lock) = data!(ctx, (Config, Database, History));

    let options = &command.data.options;

    // Parse arguments
    let message = parse_arg::<String>(options, 0)?;
    let minutes = parse_arg::<i64>(options, 1)?;
    let mut hours = 0;
    let mut days = 0;
    for i in 2..options.len() {
        match options.get(i).unwrap().name.as_str() {
            "hours" => hours = parse_arg(options, i)?,
            "days" => days = parse_arg(options, i)?,
            _ => unreachable!(),
        }
    }

    // Add message to history
    {
        let mut history = history_lock.write().await;

        history.add_entry(
            &config,
            command.user.id,
            parse_arg_name(options, 0)?,
            &message,
        );
    }

    if minutes + hours + days == 0 {
        return send_response(
            &ctx,
            &command,
            command_config,
            "Schedule reminder",
            "Why would I need to schedule a reminder if you need the reminder right now?",
        )
        .await;
    }

    // Get datetime of reminder
    let datetime = Utc::now().duration_trunc(Duration::minutes(1)).unwrap()
        + Duration::minutes(minutes)
        + Duration::hours(hours)
        + Duration::days(days);

    // Get response of the bot
    let response = command.get_interaction_response(&ctx.http).await?;

    // Add reminder to database
    database
        .client
        .execute(
            "
    INSERT INTO reminders
    VALUES ($1::BIGINT, $2::BIGINT, $3::BIGINT, $4::BIGINT, $5::TIMESTAMPTZ, $6::TEXT)
    ",
            &[
                &(command.guild_id.unwrap().0 as i64),
                &(command.channel_id.0 as i64),
                &(response.id.0 as i64),
                &(command.user.id.0 as i64),
                &datetime,
                &message,
            ],
        )
        .await?;

    send_response(
        &ctx,
        &command,
        command_config,
        "Schedule reminder",
        &format!(
            "I'm going to remind you about \"{}\" in approximately {}, {} and {}!",
            message,
            pluralize!("day", days),
            pluralize!("hour", hours),
            pluralize!("minute", minutes)
        ),
    )
    .await
}
