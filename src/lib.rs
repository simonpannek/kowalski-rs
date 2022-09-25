#![feature(decl_macro)]
#[cfg(feature = "event-calendar")]
pub mod calendar;
pub mod client;
pub mod commands;
pub mod config;
pub mod cooldowns;
pub mod credits;
pub mod database;
pub mod error;
pub mod events;
pub mod history;
#[cfg(feature = "nlp-model")]
pub mod model;
pub mod reminders;
pub mod strings;
pub mod utils;
