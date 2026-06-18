//! Parser for `adb shell pm list packages [-s|-3]` output.
//!
//! Each line is `package:<name>`. We optionally carry the system/third-party
//! distinction via the [`PackageListParser::parse_with_system`] helper, since
//! that information comes from *which* flag was passed, not from the text.

use crate::model::PackageRef;
use crate::{Parse, ParseError, ParseResult};

pub struct PackageListParser;

impl PackageListParser {
    /// Parse, tagging every entry with the given `system` flag. Callers know
    /// whether they ran `-s` (system) or `-3` (third party).
    pub fn parse_with_system(raw: &str, system: bool) -> ParseResult<Vec<PackageRef>> {
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        let pkgs = raw
            .lines()
            .filter_map(|line| line.trim().strip_prefix("package:"))
            .map(|name| PackageRef {
                name: name.trim().to_string(),
                system,
            })
            .collect();
        Ok(pkgs)
    }
}

impl Parse for PackageListParser {
    type Output = Vec<PackageRef>;

    /// Default parse assumes the `system` flag is unknown (false).
    fn parse(raw: &str) -> ParseResult<Self::Output> {
        let out = Self::parse_with_system(raw, false)?;
        if out.is_empty() && !raw.trim().is_empty() {
            return Err(ParseError::UnexpectedFormat(
                "no `package:` lines found".into(),
            ));
        }
        Ok(out)
    }
}
