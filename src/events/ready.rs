use serenity::{
    client::Context,
    model::{
        gateway::{Activity, Ready},
        id::GuildId,
        interactions::application_command::ApplicationCommand,
    },
};
use tracing::info;

use crate::{
    config::Config,
    database::{client::Database, types::ModuleStatus},
    strings::{
        ERR_CMD_CREATION, ERR_DATA_ACCESS, ERR_DB_QUERY, INFO_CMD_GLOBAL, INFO_CMD_MODULE,
        INFO_CONNECTED,
    },
    utils::{add_permissions, create_command, create_module_command},
};

pub async fn ready(ctx: &Context, rdy: Ready) {
    info!("{}", INFO_CONNECTED);

    // Set the bot status
    let activity = Activity::listening("reactions");
    ctx.set_activity(activity).await;

    // TODO: Clean up database

    setup_commands(ctx, rdy).await;
}

async fn setup_commands(ctx: &Context, rdy: Ready) {
    // Get config and database
    let (config, database) = {
        let data = ctx.data.read().await;

        let config = data.get::<Config>().expect(ERR_DATA_ACCESS).clone();
        let database = data.get::<Database>().expect(ERR_DATA_ACCESS).clone();

        (config, database)
    };

    // Create global commands
    let commands = create_global_commands(ctx, &config).await;
    // Set permissions of the global commands per guild
    for guild in rdy.guilds {
        add_permissions(ctx, &config, guild.id(), &commands).await;
    }
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
