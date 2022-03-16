use std::{
    cmp::{max, min},
    collections::HashMap,
    sync::Arc,
};

use chrono::Utc;
use serenity::prelude::TypeMapKey;
use tokio::sync::RwLock;

use crate::config::Config;

/// Credits struct containing a map, mapping user ids to their credits.
pub struct Credits {
    credits: HashMap<u64, i64>,
}

impl Credits {
    pub fn new() -> Self {
        Credits {
            credits: HashMap::new(),
        }
    }

    /// Add credits to a user
    ///
    /// Returns whether the user surpassed the threshold
    pub fn add_credits(&mut self, config: &Config, user: u64, credits: i64) -> bool {
        // Get lower credits bound
        let lower_bound = Utc::now().timestamp();
        // Update user credits
        let user_credits = self
            .credits
            .entry(user)
            .and_modify(|current| {
                // Make sure the credits do not exceed the threshold too far
                let start_value = min(*current, lower_bound + config.general.credits_margin);
                // Update current
                *current = max(lower_bound, start_value) + credits;
            })
            .or_insert(lower_bound + credits);

        *user_credits >= lower_bound + config.general.credits_margin
    }
}

impl TypeMapKey for Credits {
    type Value = Arc<RwLock<Credits>>;
}
