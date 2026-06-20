//! Parser for `adb shell cat /proc/meminfo` output.
//!
//! Lines look like `MemTotal:        3756128 kB`. We only need the two
//! totals; everything else in the dump is ignored.

use crate::model::MemoryInfo;
use crate::{Parse, ParseError, ParseResult};

pub struct MemInfoParser;

impl Parse for MemInfoParser {
    type Output = MemoryInfo;

    fn parse(raw: &str) -> ParseResult<MemoryInfo> {
        let mut total_kb = 0u64;
        let mut available_kb = 0u64;

        for line in raw.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                total_kb = parse_kb(rest);
            } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
                available_kb = parse_kb(rest);
            }
        }

        if total_kb == 0 {
            return Err(ParseError::UnexpectedFormat(
                "no MemTotal line found".into(),
            ));
        }

        Ok(MemoryInfo {
            total_kb,
            available_kb,
        })
    }
}

fn parse_kb(s: &str) -> u64 {
    s.trim()
        .split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}
