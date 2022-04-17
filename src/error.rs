use std::fmt::Debug;

use thiserror::Error;

/// Custom error type of the bot
#[derive(Error, Debug)]
pub enum KowalskiError {
    #[error("Unexpected response from the Discord API: {0}")]
    DiscordApiError(String),
    #[error("Failed to execute the database query: {source:?}")]
    DatabaseError {
        #[from]
        source: tokio_postgres::Error,
    },
    #[cfg(feature = "nlp-model")]
    #[error("Something went wrong handling the language model: {source:?}")]
    ModelError {
        #[from]
        source: rust_bert::RustBertError,
    },
}

impl From<serenity::Error> for KowalskiError {
    fn from(why: serenity::Error) -> Self {
        KowalskiError::DiscordApiError(format!("{}", why))
    }
}

impl From<serde_json::Error> for KowalskiError {
    fn from(why: serde_json::Error) -> Self {
        KowalskiError::DiscordApiError(format!("{}", why))
    }
}
