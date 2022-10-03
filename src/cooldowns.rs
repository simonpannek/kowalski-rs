use std::{cmp::min, collections::HashMap, sync::Arc};

use chrono::{DateTime, Duration, Utc};
use serenity::{
    model::id::{GuildId, RoleId, UserId},
    prelude::TypeMapKey,
};
use tokio::sync::RwLock;

use crate::{config::Config, database::client::Database, error::KowalskiError};

/// Cooldown struct containing a map, mapping guild ids to the cooldowns of the guild.
pub struct Cooldowns {
    guilds: HashMap<GuildId, GuildCooldowns>,
}

/// GuildCooldowns struct containing a map, mapping user ids to the cooldowns of the reaction.
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
        let guild_cooldowns = self.guilds.entry(guild_id).or_insert(GuildCooldowns {
            cooldowns: HashMap::new(),
        });

        let active = match guild_cooldowns.cooldowns.get(&user_id) {
            Some(&date) => date > Utc::now(),
            None => false,
        };

        // Get guild and role ids
        let guild_db_id = database.get_guild(guild_id).await?;

        // Add new cooldown if none is active
        if !active {
            let cooldown_end = {
                let mut cooldown = None;

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

                        match cooldown {
                            Some(cooldown_value) => {
                                cooldown = Some(min(cooldown_value, role_cooldown));
                            }
                            None => cooldown = Some(role_cooldown),
                        }
                    }
                }

                Utc::now() + Duration::seconds(cooldown.unwrap_or(config.general.default_cooldown))
            };

            guild_cooldowns.cooldowns.insert(user_id, cooldown_end);
        }

        Ok(active)
    }
}

impl TypeMapKey for Cooldowns {
    type Value = Arc<RwLock<Cooldowns>>;
}
