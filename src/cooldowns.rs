use std::{cmp::min, collections::HashMap, sync::Arc};

use chrono::{DateTime, Duration, Utc};
use serenity::prelude::TypeMapKey;
use tokio::sync::RwLock;

use crate::{config::Config, database::client::Database, error::ExecutionError};

pub struct Cooldowns {
    guilds: HashMap<u64, GuildCooldowns>,
}

struct GuildCooldowns {
    cooldowns: HashMap<u64, DateTime<Utc>>,
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
        guild: u64,
        user: u64,
        roles: &Vec<u64>,
    ) -> Result<bool, ExecutionError> {
        // Get or create guild cooldowns
        let guild_cooldowns = match self.guilds.get_mut(&guild) {
            Some(cooldowns) => cooldowns,
            None => {
                self.guilds.insert(
                    guild,
                    GuildCooldowns {
                        cooldowns: HashMap::new(),
                    },
                );
                self.guilds.get_mut(&guild).unwrap()
            }
        };

        let active = match guild_cooldowns.cooldowns.get(&user) {
            Some(&date) => date > Utc::now(),
            None => false,
        };

        // Add new cooldown if none is active
        if !active {
            let cooldown_end = {
                let mut cooldown = config.general.default_cooldown;

                for role in roles {
                    let row = database
                        .client
                        .query_opt(
                            "
                        SELECT cooldown FROM score_cooldowns
                        WHERE guild = $1::BIGINT AND role = $2::BIGINT
                        ",
                            &[&(guild as i64), &(*role as i64)],
                        )
                        .await?;

                    if let Some(row) = row {
                        let role_cooldown = row.get(0);
                        cooldown = min(cooldown, role_cooldown);
                    }
                }

                Utc::now() + Duration::seconds(cooldown)
            };

            guild_cooldowns.cooldowns.insert(user, cooldown_end);
        }

        Ok(active)
    }
}

impl TypeMapKey for Cooldowns {
    type Value = Arc<RwLock<Cooldowns>>;
}
