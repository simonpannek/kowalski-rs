use std::fmt::{Debug, Display, Formatter, Result};

/// Custom error type when execution something like a command.
pub struct ExecutionError {
    reason: String,
}

impl ExecutionError {
    pub fn new(reason: &str) -> Self {
        ExecutionError {
            reason: String::from(reason),
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
