//! Parser for `adb logcat -v threadtime` output.
//!
//! The `threadtime` format looks like:
//! `06-18 14:23:01.123  1234  1250 E MyTag: something failed`
//! i.e. `<date> <time> <pid> <tid> <priority> <tag>: <message>`.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::model::{LogEntry, LogPriority};
use crate::{Parse, ParseResult};

static LINE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^(?P<ts>\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\.\d{3})\s+(?P<pid>\d+)\s+(?P<tid>\d+)\s+(?P<pri>[VDIWEF])\s+(?P<tag>[^:]*?):\s?(?P<msg>.*)$",
    )
    .unwrap()
});

pub struct LogcatParser;

impl LogcatParser {
    /// Parse a single line, returning `None` for lines that aren't log entries
    /// (e.g. `--------- beginning of main` separators).
    pub fn parse_line(line: &str) -> Option<LogEntry> {
        let caps = LINE.captures(line)?;
        let pri = caps["pri"].chars().next().map(LogPriority::from_code)?;
        Some(LogEntry {
            timestamp: caps["ts"].to_string(),
            pid: caps["pid"].parse().unwrap_or(0),
            tid: caps["tid"].parse().unwrap_or(0),
            priority: pri,
            tag: caps["tag"].trim().to_string(),
            message: caps["msg"].to_string(),
        })
    }
}

impl Parse for LogcatParser {
    type Output = Vec<LogEntry>;

    fn parse(raw: &str) -> ParseResult<Self::Output> {
        Ok(raw.lines().filter_map(Self::parse_line).collect())
    }
}
