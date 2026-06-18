//! Parser for `adb shell dumpsys battery` output.
//!
//! The dump is a flat list of `  key: value` lines under a header. We read the
//! handful of fields we expose and normalize units (dumpsys reports temperature
//! in tenths of a degree Celsius).

use std::collections::HashMap;

use crate::model::{BatteryInfo, BatteryStatus};
use crate::{Parse, ParseError, ParseResult};

pub struct BatteryParser;

impl BatteryParser {
    fn to_map(raw: &str) -> HashMap<String, String> {
        raw.lines()
            .filter_map(|line| {
                let (k, v) = line.split_once(':')?;
                Some((k.trim().to_string(), v.trim().to_string()))
            })
            .collect()
    }
}

impl Parse for BatteryParser {
    type Output = BatteryInfo;

    fn parse(raw: &str) -> ParseResult<Self::Output> {
        if raw.trim().is_empty() {
            return Err(ParseError::Empty);
        }
        let m = Self::to_map(raw);

        let num = |key: &'static str| -> ParseResult<i64> {
            m.get(key)
                .ok_or(ParseError::MissingField(key))?
                .parse::<i64>()
                .map_err(|_| ParseError::InvalidValue {
                    field: key,
                    value: m.get(key).cloned().unwrap_or_default(),
                })
        };

        // `status` is an int code in dumpsys: 2=charging 3=discharging
        // 4=not charging 5=full, anything else unknown.
        let status = match num("status").unwrap_or(1) {
            2 => BatteryStatus::Charging,
            3 => BatteryStatus::Discharging,
            4 => BatteryStatus::NotCharging,
            5 => BatteryStatus::Full,
            _ => BatteryStatus::Unknown,
        };

        let power_source = if m.get("AC powered").map(|v| v == "true").unwrap_or(false) {
            "AC"
        } else if m.get("USB powered").map(|v| v == "true").unwrap_or(false) {
            "USB"
        } else if m
            .get("Wireless powered")
            .map(|v| v == "true")
            .unwrap_or(false)
        {
            "Wireless"
        } else {
            "None"
        }
        .to_string();

        Ok(BatteryInfo {
            level: num("level").unwrap_or(0) as u8,
            scale: num("scale").unwrap_or(100) as u8,
            power_source,
            status,
            health: m.get("health").cloned().unwrap_or_default(),
            temperature_c: num("temperature").unwrap_or(0) as f32 / 10.0,
            voltage_mv: num("voltage").unwrap_or(0) as u32,
        })
    }
}
