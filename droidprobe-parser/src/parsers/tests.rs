//! Unit tests run parsers against captured fixture output. No device needed.

use crate::model::{BatteryStatus, LogPriority};
use crate::parsers::{
    battery::BatteryParser, getprop::GetpropParser, logcat::LogcatParser,
    packages::PackageListParser,
};
use crate::Parse;

#[test]
fn parses_getprop() {
    let raw = "\
[ro.product.model]: [Pixel 7]
[ro.product.manufacturer]: [Google]
[ro.product.brand]: [google]
[ro.build.version.release]: [14]
[ro.build.version.sdk]: [34]
[ro.product.cpu.abi]: [arm64-v8a]
[ro.build.fingerprint]: [google/panther/panther:14/UP1A]";
    let info = GetpropParser::parse(raw).unwrap();
    assert_eq!(info.model, "Pixel 7");
    assert_eq!(info.manufacturer, "Google");
    assert_eq!(info.sdk, 34);
    assert_eq!(info.abi, "arm64-v8a");
}

#[test]
fn parses_battery() {
    let raw = "\
Current Battery Service state:
  AC powered: false
  USB powered: true
  Wireless powered: false
  status: 2
  health: 2
  level: 87
  scale: 100
  voltage: 4123
  temperature: 312";
    let b = BatteryParser::parse(raw).unwrap();
    assert_eq!(b.level, 87);
    assert_eq!(b.power_source, "USB");
    assert_eq!(b.status, BatteryStatus::Charging);
    assert!((b.temperature_c - 31.2).abs() < 0.001);
    assert_eq!(b.voltage_mv, 4123);
}

#[test]
fn parses_packages() {
    let raw = "package:com.android.settings\npackage:com.example.app\n";
    let pkgs = PackageListParser::parse_with_system(raw, false).unwrap();
    assert_eq!(pkgs.len(), 2);
    assert_eq!(pkgs[0].name, "com.android.settings");
}

#[test]
fn parses_logcat_line() {
    let line = "06-18 14:23:01.123  1234  1250 E ActivityManager: ANR in com.example";
    let entry = LogcatParser::parse_line(line).unwrap();
    assert_eq!(entry.pid, 1234);
    assert_eq!(entry.tid, 1250);
    assert_eq!(entry.priority, LogPriority::Error);
    assert_eq!(entry.tag, "ActivityManager");
    assert!(entry.message.contains("ANR"));
}

#[test]
fn skips_non_log_lines() {
    let raw = "--------- beginning of main\n06-18 14:23:01.123  1  2 I Tag: hi";
    let entries = LogcatParser::parse(raw).unwrap();
    assert_eq!(entries.len(), 1);
}
