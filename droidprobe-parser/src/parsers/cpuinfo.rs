//! Parser for `adb shell cat /proc/cpuinfo` output.
//!
//! Core count is the number of `processor : N` lines; `Hardware` is whatever
//! the SoC vendor put there (board/chipset name). CPU architecture isn't
//! parsed here — `/proc/cpuinfo` doesn't report it reliably on arm64, so
//! callers should use [`crate::model::DeviceInfo::abi`] instead.

use crate::model::CpuInfo;
use crate::{Parse, ParseResult};

pub struct CpuInfoParser;

impl Parse for CpuInfoParser {
    type Output = CpuInfo;

    fn parse(raw: &str) -> ParseResult<CpuInfo> {
        let mut core_count = 0u32;
        let mut hardware = String::new();

        for line in raw.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("processor") {
                if rest.trim_start().starts_with(':') {
                    core_count += 1;
                }
                continue;
            }
            if let Some((key, val)) = line.split_once(':') {
                if key.trim().eq_ignore_ascii_case("Hardware") {
                    hardware = val.trim().to_string();
                }
            }
        }

        Ok(CpuInfo {
            core_count,
            hardware,
        })
    }
}
