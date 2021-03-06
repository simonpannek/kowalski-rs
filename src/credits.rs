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
    /// Returns a values if the user surpassed the threshold (the optional value is the seconds left)
    pub fn add_credits(&mut self, config: &Config, user: u64, credits: i64) -> Option<i64> {
        // Get lower credits bound
        let lower_bound = Utc::now().timestamp();
        // Update user credits
        let user_credits = self
            .credits
            .entry(user)
            .and_modify(|current| {
                // Calculate the new value
                let new_value = max(lower_bound, *current) + credits;
                // Update current, make sure the credits do not exceed the threshold too far
                *current = min(new_value, lower_bound + config.general.credits_margin * 2);
            })
            .or_insert(lower_bound + credits);

        let remaining = *user_credits - lower_bound - config.general.credits_margin;

        if remaining - credits > 0 {
            Some(remaining)
        } else {
            None
        }
    }
}

impl TypeMapKey for Credits {
    type Value = Arc<RwLock<Credits>>;
}
