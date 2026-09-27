use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("Unexpected end of file at byte offset {0}")]
    UnexpectedEof(usize),

    #[error("Syntax error at offset {offset}: {message}")]
    SyntaxError {
        offset: usize,
        message: &'static str,
    },

    #[error("Invalid numeric literal at offset {0}")]
    InvalidNumber(usize),

    #[error("Invalid hexadecimal string at offset {0}")]
    InvalidHexString(usize),

    #[error("Unterminated literal string starting at offset {0}")]
    UnterminatedString(usize),

    #[error("Excessive recursion or depth limit ({0}) exceeded")]
    RecursionLimitExceeded(usize),

    #[error("I/O error: {0}")]
    Io(String),
}

pub type Result<T> = std::result::Result<T, Error>;
