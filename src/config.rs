use std::{collections::HashMap, error::Error, sync::Arc};

use linked_hash_map::LinkedHashMap;
use serde::Deserialize;
use serenity::{
    model::{
        channel::ChannelType, interactions::application_command::ApplicationCommandOptionType,
        Permissions,
    },
    prelude::TypeMapKey,
};
use strum_macros::{EnumIter, EnumString};
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
    pub interaction_timeout: u64,
    pub command_history_size: usize,
    pub autocomplete_size: usize,
    pub default_cooldown: i64,
    pub leaderboard_size: usize,
    pub leaderboard_titles: Vec<String>,
    pub credits_margin: i64,
    pub pickup_timeout: u64,
    pub nlp_max_message_length: usize,
    pub nlp_max_messages: u64,
    pub nlp_group_size: usize,
    pub reminder_list_size: usize,
    pub reminder_list_max_message_length: usize,
}

#[derive(Deserialize)]
pub struct Command {
    pub command_type: CommandType,
    pub description: String,
    pub module: Option<Module>,
    pub permission: Option<Permissions>,
    pub owner: Option<bool>,
    pub options: Option<LinkedHashMap<String, CommandOption>>,
    pub cost: Option<i64>,
}

/// Types of commands parsed by the config.
#[derive(Deserialize)]
pub enum CommandType {
    About,
    Module,
    Modules,
    Ping,
    Clean,
    Guild,
    Say,
    Sql,
    Clear,
    Reminder,
    Reminders,
    Cooldown,
    Cooldowns,
    Drop,
    Drops,
    Emoji,
    Emojis,
    Gift,
    Given,
    LevelUp,
    LevelUps,
    Moderation,
    Moderations,
    Rank,
    Score,
    Scores,
    ReactionRole,
    ReactionRoles,
    Mood,
    Oracle,
    Tldr,
}

/// Types of modules parsed by the config.
#[derive(EnumIter, EnumString, Debug, Deserialize, PartialEq)]
pub enum Module {
    Owner,
    Utility,
    Score,
    ReactionRoles,
    Analyze,
}

/// An option of a command.
#[derive(Deserialize)]
pub struct CommandOption {
    pub kind: OptionType,
    pub description: String,
    pub default: Option<bool>,
    pub required: Option<bool>,
    pub choices: Option<Vec<Value>>,
    pub options: Option<LinkedHashMap<String, CommandOption>>,
    pub channel_types: Option<Vec<Channel>>,
    pub min_value: Option<i32>,
    pub max_value: Option<i32>,
    pub autocomplete: Option<bool>,
}

/// Types of options parsed by the config
#[derive(Clone, Copy, Deserialize)]
pub enum OptionType {
    SubCommand,
    SubCommandGroup,
    String,
    Integer,
    Boolean,
    User,
    Channel,
    Role,
    Mentionable,
    Number,
}

/// Types of options parsed by the config
#[derive(Clone, Copy, Deserialize)]
pub enum Channel {
    Text,
    Private,
    Voice,
    Category,
    News,
    NewsThread,
    PublicThread,
    PrivateThread,
    Stage,
    Directory,
    Forum,
    Unknown,
}

/// A struct either representing a string or an int.
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Value {
    Int(i32),
    String(String),
}

impl Config {
    pub async fn new() -> Result<Self, Box<dyn Error>> {
        let path = "Config.toml";

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

impl Into<ApplicationCommandOptionType> for OptionType {
    fn into(self) -> ApplicationCommandOptionType {
        match self {
            OptionType::SubCommand => ApplicationCommandOptionType::SubCommand,
            OptionType::SubCommandGroup => ApplicationCommandOptionType::SubCommandGroup,
            OptionType::String => ApplicationCommandOptionType::String,
            OptionType::Integer => ApplicationCommandOptionType::Integer,
            OptionType::Boolean => ApplicationCommandOptionType::Boolean,
            OptionType::User => ApplicationCommandOptionType::User,
            OptionType::Channel => ApplicationCommandOptionType::Channel,
            OptionType::Role => ApplicationCommandOptionType::Role,
            OptionType::Mentionable => ApplicationCommandOptionType::Mentionable,
            OptionType::Number => ApplicationCommandOptionType::Number,
        }
    }
}

impl Into<ChannelType> for Channel {
    fn into(self) -> ChannelType {
        match self {
            Channel::Text => ChannelType::Text,
            Channel::Private => ChannelType::Private,
            Channel::Voice => ChannelType::Voice,
            Channel::Category => ChannelType::Category,
            Channel::News => ChannelType::News,
            Channel::NewsThread => ChannelType::NewsThread,
            Channel::PublicThread => ChannelType::PublicThread,
            Channel::PrivateThread => ChannelType::PrivateThread,
            Channel::Stage => ChannelType::Stage,
            Channel::Directory => ChannelType::Directory,
            Channel::Forum => ChannelType::Forum,
            Channel::Unknown => ChannelType::Unknown,
        }
    }
}
