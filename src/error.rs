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

    #[error("Broken cross-reference table/stream at offset {offset}: expected {expected}")]
    BrokenXref {
        offset: usize,
        expected: &'static str,
    },

    #[error("Cyclic reference detected: object {object_id} gen {generation}")]
    CyclicReference {
        object_id: u32,
        generation: u16,
    },

    #[error("Truncated file: expected offset {expected_offset}, file length is {file_len}")]
    TruncatedFile {
        expected_offset: usize,
        file_len: usize,
    },

    #[error("Unsupported filter: {name}")]
    UnsupportedFilter {
        name: String,
    },

    #[error("Missing PDF trailer")]
    MissingTrailer,

    #[error("Document repair pass failed after {attempts} recovery attempts")]
    RecoveryFailed {
        attempts: u32,
    },

    #[error("I/O error: {0}")]
    Io(String),

    #[error("Unsupported feature: {0}")]
    Unsupported(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;
