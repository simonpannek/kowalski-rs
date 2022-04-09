use std::fmt::{Debug, Display, Formatter, Result};

use crate::strings::ERR_DB_QUERY;

/// Custom error type when executing something like a command.
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

#[cfg(feature = "nlp-model")]
impl From<rust_bert::RustBertError> for ExecutionError {
    fn from(e: rust_bert::RustBertError) -> Self {
        ExecutionError::new(&format!("{:?}", e))
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
