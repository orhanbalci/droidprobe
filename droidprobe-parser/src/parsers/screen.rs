//! Parser for `adb shell wm size` + `adb shell wm density` output.
//!
//! Each command's output is a single line such as `Physical size: 1080x2340`
//! or `Physical density: 420`. Two separate shell calls are needed (`wm`
//! doesn't expose both in one invocation), so this parser takes both raw
//! strings at once rather than implementing the single-string [`crate::Parse`]
//! trait.

use crate::model::ScreenInfo;
use crate::ParseResult;

pub struct ScreenInfoParser;

impl ScreenInfoParser {
    pub fn parse_combined(size_raw: &str, density_raw: &str) -> ParseResult<ScreenInfo> {
        let (width, height) = size_raw
            .lines()
            .find_map(|l| l.split_once(':').map(|(_, v)| v.trim()))
            .and_then(|dims| dims.split_once(['x', 'X']))
            .map(|(w, h)| (w.trim().parse().unwrap_or(0), h.trim().parse().unwrap_or(0)))
            .unwrap_or((0, 0));

        let density_dpi = density_raw
            .lines()
            .find_map(|l| l.split_once(':').map(|(_, v)| v.trim().parse::<u32>().ok()))
            .flatten()
            .unwrap_or(0);

        Ok(ScreenInfo {
            width,
            height,
            density_dpi,
        })
    }
}
