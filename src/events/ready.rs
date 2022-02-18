use serenity::{
    client::Context,
    model::{
        gateway::{Activity, Ready},
        id::GuildId,
        interactions::application_command::{
            ApplicationCommand,
            ApplicationCommandPermissionType::{Role, User},
        },
    },
};
use tracing::info;

use crate::{
    config::{Config, Module},
    database::{client::Database, types::ModuleStatus},
    strings::{
        ERR_API_LOAD, ERR_CMD_CREATION, ERR_CMD_NOT_FOUND, ERR_CMD_SET_PERMISSION, ERR_DATA_ACCESS,
        ERR_DB_QUERY, INFO_CMD_GLOBAL, INFO_CMD_MODULE, INFO_CONNECTED,
    },
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
        add_permissions(guild.id(), &commands, &config, ctx).await;
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
            commands.create_application_command(|command| {
                command
                    .name(name)
                    .description(&options.description)
                    .default_permission(options.default_permission)
            });
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

        // Filter commands for the configuration of the current guild
        let filtered = config
            .commands
            .iter()
            .filter(|(_, options)| match &options.module {
                Some(module) => match module {
                    Module::Owner => status.owner,
                    Module::Utility => status.utility,
                    Module::Reactions => status.reactions,
                    Module::ReactionRoles => status.reaction_roles,
                },
                None => false,
            });

        // Add the commands
        let commands = guild
            .set_application_commands(&ctx.http, |commands| {
                for (name, options) in filtered {
                    commands.create_application_command(|command| {
                        command
                            .name(name)
                            .description(&options.description)
                            .default_permission(options.default_permission)
                    });
                }

                commands
            })
            .await
            .expect(ERR_CMD_CREATION);

        add_permissions(guild, &commands, &config, ctx).await;
    }
}

async fn add_permissions(
    guild: GuildId,
    commands: &Vec<ApplicationCommand>,
    config: &Config,
    ctx: &Context,
) {
    // Get the partial guild to get the owner information later
    let partial_guild = guild.to_partial_guild(&ctx.http).await.expect(ERR_API_LOAD);

    // Get commands which do not have default permissions
    let commands = commands
        .iter()
        .filter(|command| !command.default_permission);

    for command in commands {
        // Get config of the command
        let command_config = config.commands.get(&command.name).expect(ERR_CMD_NOT_FOUND);

        // Get roles which should have access to the command
        let roles: Option<Vec<_>> = match command_config.permission {
            Some(permission) => Some(
                guild
                    .roles(&ctx.http)
                    .await
                    .expect(ERR_API_LOAD)
                    .iter()
                    .filter(|(_, role)| role.permissions.contains(permission))
                    .map(|(&id, _)| u64::from(id))
                    .collect(),
            ),
            None => None,
        };

        guild
            .create_application_command_permission(&ctx.http, command.id, |command_perms| {
                // Set owner execution only
                if command_config.owner.unwrap_or_default() {
                    for &owner in &config.general.owners {
                        command_perms
                            .create_permission(|perm| perm.kind(User).id(owner).permission(true));
                    }
                }

                // Set custom permissions
                if command_config.permission.is_some() {
                    // Always give permission to the guild owner
                    command_perms.create_permission(|perm| {
                        perm.kind(User)
                            .id(u64::from(partial_guild.owner_id))
                            .permission(true)
                    });

                    // TODO: Listen for guild owner change and role edit events

                    // Set custom permission for roles with the permission
                    if let Some(roles) = roles {
                        for id in roles {
                            command_perms
                                .create_permission(|perm| perm.kind(Role).id(id).permission(true));
                        }
                    };
                }

                command_perms
            })
            .await
            .expect(ERR_CMD_SET_PERMISSION);
    }
}
