use crate::strings::ERR_DB_QUERY;
use std::fmt::{Debug, Display, Formatter, Result};

/// Custom error type when execution something like a command.
pub struct ExecutionError {
    reason: String,
}

impl ExecutionError {
    pub fn new(reason: &str) -> Self {
        ExecutionError {
            reason: reason.to_string(),
        }
    }
}

impl Debug for ExecutionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{{ reason: {}, file: {}, line: {} }}",
            self.reason,
            file!(),
            line!()
        )
    }
}

impl Display for ExecutionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.reason)
    }
}

impl From<serenity::Error> for ExecutionError {
    fn from(e: serenity::Error) -> Self {
        ExecutionError::new(&format!("{}", e))
    }
}

impl From<tokio_postgres::Error> for ExecutionError {
    fn from(e: tokio_postgres::Error) -> Self {
        ExecutionError::new(&format!("{}: {}", ERR_DB_QUERY, e))
    }
}
