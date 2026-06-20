//! Parser for `adb shell df` output.
//!
//! Standard six-column `df` layout:
//! `Filesystem  1K-blocks  Used  Available  Use%  Mounted on`. The header
//! line is skipped; any data line that doesn't split into at least 6
//! whitespace-separated fields is skipped too rather than failing the whole
//! parse, since some Android `df` builds emit extra banner lines.
//!
//! Toybox's `df` (the one on most Android builds) defaults to human-readable
//! sizes like `1.9G` or `84K` rather than raw 1K-blocks, unlike GNU `df`. Size
//! columns are parsed leniently to handle either form.

use crate::model::StorageEntry;
use crate::{Parse, ParseResult};

pub struct StorageParser;

impl Parse for StorageParser {
    type Output = Vec<StorageEntry>;

    fn parse(raw: &str) -> ParseResult<Vec<StorageEntry>> {
        let entries = raw
            .lines()
            .skip(1) // header: "Filesystem  1K-blocks  Used  Available  Use% Mounted on"
            .filter_map(|line| {
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.len() < 6 {
                    return None;
                }
                Some(StorageEntry {
                    filesystem: cols[0].to_string(),
                    size_kb: parse_size_kb(cols[1]),
                    used_kb: parse_size_kb(cols[2]),
                    available_kb: parse_size_kb(cols[3]),
                    use_percent: cols[4].trim_end_matches('%').parse().unwrap_or(0),
                    mounted_on: cols[5..].join(" "),
                })
            })
            .collect();
        Ok(entries)
    }
}

/// Parses a `df` size column into kilobytes. Accepts either a bare number
/// (already in 1K-blocks) or a human-readable size with a K/M/G/T suffix.
fn parse_size_kb(s: &str) -> u64 {
    let s = s.trim();
    if let Ok(n) = s.parse::<u64>() {
        return n;
    }
    let Some(last) = s.chars().last() else {
        return 0;
    };
    let multiplier = match last.to_ascii_uppercase() {
        'K' => 1.0,
        'M' => 1024.0,
        'G' => 1024.0 * 1024.0,
        'T' => 1024.0 * 1024.0 * 1024.0,
        _ => return 0,
    };
    s[..s.len() - 1]
        .trim()
        .parse::<f64>()
        .map(|n| (n * multiplier) as u64)
        .unwrap_or(0)
}
