use std::{cmp::min, collections::HashMap, sync::Arc};

use chrono::{DateTime, Duration, Utc};
use serenity::{prelude::TypeMapKey, model::id::{GuildId, RoleId, UserId}};
use tokio::sync::RwLock;

use crate::{config::Config, database::client::Database, error::KowalskiError};

/// Cooldown struct containing a map, mapping guild ids to the cooldowns of the guild.
pub struct Cooldowns {
    guilds: HashMap<GuildId, GuildCooldowns>,
}

/// GuildCooldowns struct containing a map, mapping user ids to the cooldowns of the command.
struct GuildCooldowns {
    cooldowns: HashMap<UserId, DateTime<Utc>>,
}

impl Cooldowns {
    pub fn new() -> Self {
        Cooldowns {
            guilds: HashMap::new(),
        }
    }

    /// Check whether the user currently has a cooldown active.
    ///
    /// Note: This will start a new cooldown, if no cooldown is currently active.
    pub async fn check_cooldown(
        &mut self,
        config: &Config,
        database: &Database,
        guild_id: GuildId,
        user_id: UserId,
        roles: &[RoleId],
    ) -> Result<bool, KowalskiError> {
        // Get or create guild cooldowns
        let guild_cooldowns = match self.guilds.get_mut(&guild_id) {
            Some(cooldowns) => cooldowns,
            None => {
                self.guilds.insert(
                    guild_id,
                    GuildCooldowns {
                        cooldowns: HashMap::new(),
                    },
                );
                self.guilds.get_mut(&guild_id).unwrap()
            }
        };

        let active = match guild_cooldowns.cooldowns.get(&user_id) {
            Some(&date) => date > Utc::now(),
            None => false,
        };

        // Get guild and role ids
        let guild_db_id = database.get_guild(guild_id).await?;

        // Add new cooldown if none is active
        if !active {
            let cooldown_end = {
                let mut cooldown = config.general.default_cooldown;

                for &role_id in roles {
                    let role_db_id = database.get_role(guild_id, role_id).await?;

                    let row = database
                        .client
                        .query_opt(
                            "
                        SELECT cooldown
                        FROM score_cooldowns
                        WHERE guild = $1::BIGINT AND role = $2::BIGINT
                        ",
                            &[&guild_db_id, &role_db_id],
                        )
                        .await?;

                    if let Some(row) = row {
                        let role_cooldown = row.get(0);
                        cooldown = min(cooldown, role_cooldown);
                    }
                }

                Utc::now() + Duration::seconds(cooldown)
            };

            guild_cooldowns.cooldowns.insert(user_id, cooldown_end);
        }

        Ok(active)
    }
}

impl TypeMapKey for Cooldowns {
    type Value = Arc<RwLock<Cooldowns>>;
}
