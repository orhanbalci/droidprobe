use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    /// The underlying adb transport failed (no device, connection dropped, etc).
    #[error("transport error: {0}")]
    Transport(String),

    /// The command ran but its output failed to parse.
    #[error("parse error: {0}")]
    Parse(#[from] droidprobe_parser::ParseError),

    /// A required argument was missing or malformed.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// A mutating command was requested but execution is in read-only mode.
    #[error("command `{0}` is mutating and read-only mode is enabled")]
    ReadOnlyViolation(String),

    /// Serialization of a typed output to JSON failed.
    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub type CommandResult<T> = Result<T, CommandError>;
