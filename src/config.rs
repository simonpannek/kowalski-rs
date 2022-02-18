use std::{collections::HashMap, error::Error, sync::Arc};

use serde::Deserialize;
use serenity::{model::Permissions, prelude::TypeMapKey};
use tokio::{fs::File, io::AsyncReadExt};

use crate::strings::{ERR_CONFIG_PARSE, ERR_CONFIG_READ};

#[derive(Deserialize)]
pub struct Config {
    pub general: General,
    pub commands: HashMap<String, Command>,
}

#[derive(Deserialize)]
pub struct General {
    pub owners: Vec<u64>,
}

#[derive(Deserialize)]
pub struct Command {
    pub command_type: CommandType,
    pub description: String,
    pub module: Option<Module>,
    pub default_permission: bool,
    pub permission: Option<Permissions>,
    pub owner: Option<bool>,
}

/// Types of commands parsed by the config.
#[derive(Deserialize)]
pub enum CommandType {
    Ping,
    About,
    Module,
    Sql,
}

/// Types of modules parsed by the config.
#[derive(Debug, Deserialize)]
pub enum Module {
    Owner,
    Utility,
    Reactions,
    ReactionRoles,
}

impl Config {
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let path = "Settings.toml";

        let mut toml = String::new();
        let mut file = File::open(path).await?;

        file.read_to_string(&mut toml)
            .await
            .expect(&format!("{}: {}", ERR_CONFIG_READ, path));

        Ok(toml::from_str(&toml).expect(ERR_CONFIG_PARSE))
    }
}

impl TypeMapKey for Config {
    type Value = Arc<Config>;
}
