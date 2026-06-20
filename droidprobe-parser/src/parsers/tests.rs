//! Unit tests run parsers against captured fixture output. No device needed.

use crate::model::{BatteryStatus, LogPriority, ProtectionLevel};
use crate::parsers::{
    battery::BatteryParser, cpuinfo::CpuInfoParser, getprop::GetpropParser, imei::ImeiInfoParser,
    logcat::LogcatParser, meminfo::MemInfoParser, package_dump::PackageDumpParser,
    packages::PackageListParser, screen::ScreenInfoParser, storage::StorageParser,
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
fn getprop_falls_back_to_abilist() {
    // Treble-ized devices often leave ro.product.cpu.abi unset and only
    // populate the abilist (primary ABI first).
    let raw = "\
[ro.product.model]: [SM-P610]
[ro.product.cpu.abilist]: [arm64-v8a,armeabi-v7a,armeabi]";
    let info = GetpropParser::parse(raw).unwrap();
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

#[test]
fn parses_package_dump() {
    let raw = "\
  Package [com.example.app] (3a2b1c4):
    userId=10123
    versionCode=318 minSdk=24 targetSdk=34
    versionName=3.2.1
    requested permissions:
      android.permission.CAMERA
      android.permission.READ_CONTACTS
      android.permission.INTERNET
    install permissions:
      android.permission.INTERNET: granted=true
    User 0: ceDataInode=1234 installed=true hidden=false
      runtime permissions:
        android.permission.CAMERA: granted=true
        android.permission.READ_CONTACTS: granted=false";
    let detail = PackageDumpParser::parse(raw).unwrap();
    assert_eq!(detail.name, "com.example.app");
    assert_eq!(detail.version_name, "3.2.1");
    assert_eq!(detail.version_code, 318);
    assert_eq!(detail.min_sdk, 24);
    assert_eq!(detail.target_sdk, 34);
    assert_eq!(detail.permissions.len(), 3);

    let camera = detail
        .permissions
        .iter()
        .find(|p| p.name == "android.permission.CAMERA")
        .unwrap();
    assert!(camera.granted);
    assert_eq!(camera.protection_level, ProtectionLevel::Dangerous);

    let contacts = detail
        .permissions
        .iter()
        .find(|p| p.name == "android.permission.READ_CONTACTS")
        .unwrap();
    assert!(!contacts.granted);
    assert_eq!(contacts.protection_level, ProtectionLevel::Dangerous);

    let internet = detail
        .permissions
        .iter()
        .find(|p| p.name == "android.permission.INTERNET")
        .unwrap();
    assert!(internet.granted);
    assert_eq!(internet.protection_level, ProtectionLevel::Normal);
}

#[test]
fn parses_cpuinfo() {
    let raw = "\
processor       : 0
BogoMIPS        : 38.40
processor       : 1
BogoMIPS        : 38.40
processor       : 2
BogoMIPS        : 38.40
processor       : 3
BogoMIPS        : 38.40
Hardware        : Qualcomm Technologies, Inc SDM660
Revision        : 0000";
    let cpu = CpuInfoParser::parse(raw).unwrap();
    assert_eq!(cpu.core_count, 4);
    assert_eq!(cpu.hardware, "Qualcomm Technologies, Inc SDM660");
}

#[test]
fn parses_screen() {
    let size = "Physical size: 1080x2340";
    let density = "Physical density: 420";
    let screen = ScreenInfoParser::parse_combined(size, density).unwrap();
    assert_eq!(screen.width, 1080);
    assert_eq!(screen.height, 2340);
    assert_eq!(screen.density_dpi, 420);
}

#[test]
fn parses_meminfo() {
    let raw = "\
MemTotal:        3756128 kB
MemFree:          123456 kB
MemAvailable:    1234567 kB
Buffers:            4567 kB";
    let mem = MemInfoParser::parse(raw).unwrap();
    assert_eq!(mem.total_kb, 3756128);
    assert_eq!(mem.available_kb, 1234567);
}

#[test]
fn parses_storage() {
    let raw = "\
Filesystem      1K-blocks    Used Available Use% Mounted on
/dev               123456   12345    111111   2% /dev
/data            58312000 1234560  45000000   3% /data";
    let entries = StorageParser::parse(raw).unwrap();
    assert_eq!(entries.len(), 2);
    let data = entries.iter().find(|e| e.mounted_on == "/data").unwrap();
    assert_eq!(data.size_kb, 58312000);
    assert_eq!(data.used_kb, 1234560);
    assert_eq!(data.available_kb, 45000000);
    assert_eq!(data.use_percent, 3);
}

#[test]
fn parses_storage_human_readable() {
    // Toybox's `df` on most real Android builds reports sizes like this by
    // default, not raw 1K-blocks.
    let raw = "\
Filesystem      Size  Used Avail Use% Mounted on
/dev             1.9G   84K  1.9G   1% /dev
/data             55G   12G   43G  22% /data";
    let entries = StorageParser::parse(raw).unwrap();
    let data = entries.iter().find(|e| e.mounted_on == "/data").unwrap();
    assert_eq!(data.size_kb, 55 * 1024 * 1024);
    assert_eq!(data.used_kb, 12 * 1024 * 1024);
    assert_eq!(data.available_kb, 43 * 1024 * 1024);
    assert_eq!(data.use_percent, 22);

    let dev = entries.iter().find(|e| e.mounted_on == "/dev").unwrap();
    assert_eq!(dev.used_kb, 84);
}

#[test]
fn parses_imei() {
    let raw = "\
Phone Subscriber Info:
  Phone Type = GSM
  Device ID = 359268000000000";
    let imei = ImeiInfoParser::parse(raw).unwrap();
    assert_eq!(imei.imei, "359268000000000");
}

#[test]
fn imei_unavailable_without_permission() {
    let raw = "Permission Denial: can't dump iphonesubinfo";
    let imei = ImeiInfoParser::parse(raw).unwrap();
    assert_eq!(imei.imei, "");
}

/// Captured from a real `adb shell dumpsys package com.spotify.music` on a
/// Samsung Android 14 device. Confirms this build never prints an
/// `exported=` field anywhere — that's why [`crate::model::Component`]
/// doesn't have one.
const SPOTIFY_DUMP: &str = "\
  Activity Resolver Table:
  Non-Data Actions:
      android.intent.action.MAIN:
        32e7ca4 com.spotify.music/com.google.android.archive.ReactivateActivity filter f9d3a0d
          Action: \"android.intent.action.MAIN\"
          Category: \"android.intent.category.LAUNCHER\"

Receiver Resolver Table:
  Non-Data Actions:
      android.intent.action.MY_PACKAGE_REPLACED:
        afddc2 com.spotify.music/com.google.android.archive.UpdateBroadcastReceiver filter 67eb7d3
          Action: \"android.intent.action.MY_PACKAGE_REPLACED\"

Domain verification status:

Permissions:
  Permission [com.spotify.music.permission.SECURED_BROADCAST] (fe54118):
    sourcePackage=com.spotify.music
    uid=10290 gids=[] type=0 prot=signature
    perm=PermissionInfo{e073641 com.spotify.music.permission.SECURED_BROADCAST}

Key Set Manager:
  [com.spotify.music]
      Signing KeySets: 52

Packages:
  Package [com.spotify.music] (1dd8540):
    userId=10290
    versionCode=110104295 minSdk=21 targetSdk=34
    versionName=8.9.8.545
    declared permissions:
      com.spotify.music.permission.SECURED_BROADCAST: prot=signature, INSTALLED
    User 0: ceDataInode=13574 installed=true hidden=false
      firstInstallTime=2023-07-25 18:54:43
      runtime permissions:

Queries:
  system apps queryable: false";

#[test]
fn parses_real_spotify_dump_components() {
    let detail = PackageDumpParser::parse(SPOTIFY_DUMP).unwrap();
    assert_eq!(detail.name, "com.spotify.music");
    assert_eq!(detail.version_name, "8.9.8.545");
    assert_eq!(detail.version_code, 110104295);
    assert_eq!(
        detail.launcher_activity.as_deref(),
        Some("com.google.android.archive.ReactivateActivity")
    );

    assert_eq!(detail.activities.len(), 1);
    assert_eq!(detail.receivers.len(), 1);
    assert!(detail.services.is_empty());
    // Providers never show up in resolver tables (no intent-filter), so this
    // stays empty even though the real app surely has one — a known gap.
    assert!(detail.providers.is_empty());

    let receiver = &detail.receivers[0];
    assert_eq!(
        receiver.name,
        "com.google.android.archive.UpdateBroadcastReceiver"
    );
    assert_eq!(
        receiver.intent_actions,
        vec!["android.intent.action.MY_PACKAGE_REPLACED"]
    );
    assert_eq!(receiver.permission, None);
}

/// Excerpt from a real `dumpsys package com.google.android.videos` dump,
/// including a permission-gated service and a stray U+FFFD byte right after
/// a `pkg/` slash (an artifact seen in the wild from this device/transport)
/// that splits the component name into its own whitespace-separated token.
const VIDEOS_DUMP_EXCERPT: &str = "\
Activity Resolver Table:
  Non-Data Actions:
      android.intent.action.MAIN:
        f560acc com.google.android.videos/.GoogleTvEntryPoint filter 4b4f115
          Action: \"android.intent.action.MAIN\"
          Category: \"android.intent.category.DEFAULT\"
          Category: \"android.intent.category.LAUNCHER\"
      com.google.android.apps.googletv.ACTION_ENTITY_PAGE:
        2d2682 com.google.android.videos/\u{FFFD}  com.google.android.apps.googletv.app.presentation.pages.entity.EntityPageActivity filter f813193
          Action: \"com.google.android.apps.googletv.ACTION_ENTITY_PAGE\"
          Category: \"android.intent.category.DEFAULT\"

Service Resolver Table:
  Non-Data Actions:
      android.service.quicksettings.action.QS_TILE:
        9ce59db com.google.android.videos/com.google.android.apps.googletv.app.device.virtualremote.ui.QuickSettingTileService filter 69f2c78 permission android.permission.BIND_QUICK_SETTINGS_TILE
          Action: \"android.service.quicksettings.action.QS_TILE\"

Permissions:";

#[test]
fn parses_videos_dump_with_corrupted_byte_and_permission() {
    let detail = PackageDumpParser::parse(VIDEOS_DUMP_EXCERPT).unwrap();
    assert_eq!(
        detail.launcher_activity.as_deref(),
        Some(".GoogleTvEntryPoint")
    );
    assert_eq!(detail.activities.len(), 2);
    let entity_page = detail
        .activities
        .iter()
        .find(|c| c.name.ends_with("EntityPageActivity"))
        .expect("corrupted-byte component name should still parse");
    assert_eq!(
        entity_page.name,
        "com.google.android.apps.googletv.app.presentation.pages.entity.EntityPageActivity"
    );

    assert_eq!(detail.services.len(), 1);
    let tile_service = &detail.services[0];
    assert_eq!(
        tile_service.permission.as_deref(),
        Some("android.permission.BIND_QUICK_SETTINGS_TILE")
    );
}
