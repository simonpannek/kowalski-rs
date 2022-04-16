use chrono::{Duration, DurationRound, Utc};
use serenity::{
    client::Context, model::interactions::application_command::ApplicationCommandInteraction,
};

use crate::{
    config::Command,
    config::Config,
    database::client::Database,
    error::ExecutionError,
    history::History,
    pluralize,
    strings::{ERR_CMD_ARGS_INVALID, ERR_DATA_ACCESS},
    utils::{parse_arg, parse_arg_name, send_response},
};

pub async fn execute(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    command_config: &Command,
) -> Result<(), ExecutionError> {
    // Get config, database and lock to history
    let (config, database, history_lock) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().expect(ERR_DATA_ACCESS).clone();
        let database = data.get::<Database>().expect(ERR_DATA_ACCESS).clone();
        let history_lock = data.get::<History>().expect(ERR_DATA_ACCESS).clone();

        (config, database, history_lock)
    };

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
            _ => return Err(ExecutionError::new(ERR_CMD_ARGS_INVALID)),
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
                &i64::from(command.guild_id.unwrap()),
                &i64::from(command.channel_id),
                &i64::from(response.id),
                &i64::from(command.user.id),
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
