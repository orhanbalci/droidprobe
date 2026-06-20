//! Parser for `adb shell dumpsys iphonesubinfo` output.
//!
//! On Android 10+ this dump requires `READ_PRIVILEGED_PHONE_STATE`, which the
//! `shell` user doesn't hold — it commonly returns a permission-denial
//! message instead of a `Device ID = ...` line. That's not a parse error,
//! just an empty IMEI; callers should render that as "unavailable".

use crate::model::ImeiInfo;
use crate::{Parse, ParseResult};

pub struct ImeiInfoParser;

impl Parse for ImeiInfoParser {
    type Output = ImeiInfo;

    fn parse(raw: &str) -> ParseResult<ImeiInfo> {
        let imei = raw
            .lines()
            .find_map(|l| l.trim().strip_prefix("Device ID = "))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        Ok(ImeiInfo { imei })
    }
}
