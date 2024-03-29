use std::time::Duration;

use serenity::{
    client::Context,
    model::{
        gateway::{Activity, Ready},
        id::GuildId,
        interactions::application_command::ApplicationCommand,
    },
};
use tracing::info;

#[cfg(feature = "event-calendar")]
use crate::calendar::host_calendar;
use crate::{
    config::Config,
    data,
    database::{client::Database, types::ModuleStatus},
    reminders::check_reminders,
    strings::{ERR_CMD_CREATION, ERR_DB_QUERY, INFO_CMD_GLOBAL, INFO_CMD_MODULE, INFO_CONNECTED},
    utils::{create_command, create_module_command},
};

pub async fn ready(ctx: &Context, rdy: Ready) {
    info!("{}", INFO_CONNECTED);

    // Set the bot status
    let activity = Activity::listening("reactions");
    ctx.set_activity(activity).await;

    // Repeatedly check for reminders
    check_reminders(ctx.clone(), Duration::from_secs(60));

    // Activate the event calendar
    #[cfg(feature = "event-calendar")]
    host_calendar(ctx.clone());

    setup_commands(ctx, rdy).await;
}

async fn setup_commands(ctx: &Context, _rdy: Ready) {
    // Get config and database
    let (config, database) = data!(ctx, (Config, Database));

    // Create global commands
    create_global_commands(ctx, &config).await;
    info!("{}", INFO_CMD_GLOBAL);

    // Create module commands per guild
    create_module_commands(ctx, &config, &database).await;
    info!("{}", INFO_CMD_MODULE);
}

async fn create_global_commands(ctx: &Context, config: &Config) -> Vec<ApplicationCommand> {
    // Get commands without a module
    let filtered = config
        .commands
        .iter()
        .filter(|(_, options)| options.module.is_none());

    ApplicationCommand::set_global_application_commands(&ctx.http, |commands| {
        for (name, options) in filtered {
            let command = create_command(name, options);
            commands.add_application_command(command);
        }

        commands
    })
    .await
    .expect(ERR_CMD_CREATION)
}

async fn create_module_commands(ctx: &Context, config: &Config, database: &Database) {
    let modules = database
        .client
        .query("SELECT * FROM modules", &[])
        .await
        .expect(ERR_DB_QUERY);

    for row in modules {
        let guild = GuildId(row.get::<_, i64>(0) as u64);
        let status: ModuleStatus = row.get(1);

        create_module_command(ctx, config, guild, &status).await
    }
}
