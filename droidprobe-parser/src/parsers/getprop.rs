//! Parser for `adb shell getprop` output.
//!
//! `getprop` emits lines shaped like `[ro.product.model]: [Pixel 7]`. We build
//! a key->value map and pick out the properties we care about for [`DeviceInfo`].
//!
//! `ro.product.cpu.abi` is the classic prop, but Treble-ized devices often
//! leave it unset and only populate `ro.product.cpu.abilist` (a comma list,
//! primary ABI first), so the ABI lookup falls back to that.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::model::DeviceInfo;
use crate::{Parse, ParseError, ParseResult};

static PROP_LINE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\[(?P<key>[^\]]+)\]:\s*\[(?P<val>.*)\]$").unwrap());

/// Parses full `getprop` output into a [`DeviceInfo`].
pub struct GetpropParser;

impl GetpropParser {
    /// Parse the raw output into a flat key/value property map.
    pub fn to_map(raw: &str) -> HashMap<String, String> {
        raw.lines()
            .filter_map(|line| {
                let caps = PROP_LINE.captures(line.trim())?;
                Some((caps["key"].to_string(), caps["val"].to_string()))
            })
            .collect()
    }
}

impl Parse for GetpropParser {
    type Output = DeviceInfo;

    fn parse(raw: &str) -> ParseResult<Self::Output> {
        if raw.trim().is_empty() {
            return Err(ParseError::Empty);
        }
        let map = Self::to_map(raw);
        let get = |k: &str| map.get(k).cloned().unwrap_or_default();

        let sdk = map
            .get("ro.build.version.sdk")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let abi = {
            let direct = get("ro.product.cpu.abi");
            if !direct.is_empty() {
                direct
            } else {
                map.get("ro.product.cpu.abilist")
                    .and_then(|list| list.split(',').next())
                    .unwrap_or_default()
                    .to_string()
            }
        };

        Ok(DeviceInfo {
            model: get("ro.product.model"),
            manufacturer: get("ro.product.manufacturer"),
            brand: get("ro.product.brand"),
            android_release: get("ro.build.version.release"),
            sdk,
            fingerprint: get("ro.build.fingerprint"),
            abi,
            serial: get("ro.serialno"),
        })
    }
}
