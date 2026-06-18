use thiserror::Error;

/// Errors that can occur while parsing command output.
#[derive(Debug, Error)]
pub enum ParseError {
    /// A required field or section was not found in the output.
    #[error("missing expected field: {0}")]
    MissingField(&'static str),

    /// A value was found but could not be converted to the expected type.
    #[error("failed to parse value for `{field}`: {value:?}")]
    InvalidValue {
        field: &'static str,
        value: String,
    },

    /// The overall shape of the input did not match what the parser expected.
    #[error("unexpected output format: {0}")]
    UnexpectedFormat(String),

    /// The input was empty when content was required.
    #[error("empty input")]
    Empty,
}

pub type ParseResult<T> = Result<T, ParseError>;
