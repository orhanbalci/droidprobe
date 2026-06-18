//! # droidprobe-parser
//!
//! Pure, side-effect-free parsing of textual `adb` / `dumpsys` / `getprop`
//! output into structured Rust types. This crate runs **no** commands and
//! performs **no** I/O — it only transforms `&str` into typed data. That makes
//! every parser trivially unit-testable against captured fixture strings.
//!
//! The central abstraction is [`Parse`]: implement it for an output type and
//! the `droidprobe-command` crate can pair a runnable command with the parser
//! that decodes its output.

pub mod error;
pub mod model;
pub mod parsers;

pub use error::{ParseError, ParseResult};

/// Transforms raw command output text into a structured value `T`.
///
/// Implementors must be pure: same input string always yields the same result,
/// and no I/O is performed. This keeps parsing testable in isolation from the
/// device and the ADB transport.
pub trait Parse {
    /// The structured type produced by this parser.
    type Output;

    /// Parse `raw` (the stdout of some adb command) into [`Self::Output`].
    fn parse(raw: &str) -> ParseResult<Self::Output>;
}
