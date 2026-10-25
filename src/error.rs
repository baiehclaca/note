use std::fmt;

/// Every failure `note` reports. The message is shown to the user as-is.
#[derive(Debug, PartialEq, Eq)]
pub enum NoteError {
    /// The command line was wrong. Exit code 2.
    Usage(String),
    /// There is no note, or no folder matched. Exit code 1.
    Missing(String),
    /// Anything else went wrong. Exit code 1.
    Failed(String),
}

impl NoteError {
    pub fn code(&self) -> i32 {
        match self {
            NoteError::Usage(_) => 2,
            NoteError::Missing(_) | NoteError::Failed(_) => 1,
        }
    }
}

impl fmt::Display for NoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NoteError::Usage(message)
            | NoteError::Missing(message)
            | NoteError::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for NoteError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_errors_exit_with_two() {
        assert_eq!(NoteError::Usage("x".into()).code(), 2);
        assert_eq!(NoteError::Missing("x".into()).code(), 1);
        assert_eq!(NoteError::Failed("x".into()).code(), 1);
        assert_eq!(NoteError::Failed("shown".into()).to_string(), "shown");
    }
}
