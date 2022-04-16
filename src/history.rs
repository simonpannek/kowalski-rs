use std::{collections::HashMap, sync::Arc};

use serenity::{model::id::UserId, prelude::TypeMapKey};
use tokio::sync::RwLock;

use crate::config::Config;

/// History struct containing a map, mapping user ids and option names to the command history of the user.
pub struct History {
    histories: HashMap<(u64, String), Vec<String>>,
}

impl History {
    pub fn new() -> Self {
        History {
            histories: HashMap::new(),
        }
    }

    pub fn add_entry(&mut self, config: &Config, user: UserId, option_name: &str, entry: &str) {
        let entry = entry.trim().to_string();
        let key = (user.0, option_name.to_string());

        let vector = match self.histories.get_mut(&key) {
            Some(vector) => vector,
            None => {
                // Insert a new vector
                self.histories.insert(key.clone(), Vec::new());
                self.histories.get_mut(&key).unwrap()
            }
        };

        // Find possible duplicate elements
        let position = vector
            .iter()
            .position(|string| string.to_lowercase() == entry.to_lowercase());

        // Insert entry to history
        match position {
            Some(position) => {
                // Insert and remove duplicate element
                vector.insert(0, entry);
                vector.remove(position + 1);
            }
            None => {
                // Just insert
                vector.insert(0, entry);
            }
        }

        // Remove last element if it exceeds the maximum history size
        if vector.len() > config.general.command_history_size {
            vector.remove(vector.len() - 1);
        }
    }

    pub fn get_entries(&self, user_id: UserId, option_name: &str) -> &[String] {
        let key = (user_id.0, option_name.to_string());

        match self.histories.get(&key) {
            Some(vector) => vector.as_slice(),
            None => &[],
        }
    }
}

impl TypeMapKey for History {
    type Value = Arc<RwLock<History>>;
}
