//! Structured output types produced by the parsers.
//!
//! These are the canonical, serializable representations of device state.
//! Both the TUI and the MCP server consume these types — the TUI renders them
//! as widgets, the MCP server serializes them to JSON for agents.

use serde::{Deserialize, Serialize};

/// High-level device identity, assembled from `getprop`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceInfo {
    pub model: String,
    pub manufacturer: String,
    pub brand: String,
    pub android_release: String,
    pub sdk: u32,
    pub fingerprint: String,
    pub abi: String,
    pub serial: String,
}

/// CPU summary from `/proc/cpuinfo`. Architecture is reported separately via
/// [`DeviceInfo::abi`] — `/proc/cpuinfo` doesn't carry it reliably on arm64.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CpuInfo {
    pub core_count: u32,
    pub hardware: String,
}

/// Screen geometry from `wm size` + `wm density`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScreenInfo {
    pub width: u32,
    pub height: u32,
    pub density_dpi: u32,
}

/// RAM totals from `/proc/meminfo`, in kilobytes as reported by the kernel.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryInfo {
    pub total_kb: u64,
    pub available_kb: u64,
}

/// One mounted filesystem row from `df`, sizes in kilobytes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageEntry {
    pub filesystem: String,
    pub size_kb: u64,
    pub used_kb: u64,
    pub available_kb: u64,
    pub use_percent: u8,
    pub mounted_on: String,
}

/// IMEI from `dumpsys iphonesubinfo`. Often empty on Android 10+ since that
/// dump requires a privileged permission `adb shell` doesn't hold.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImeiInfo {
    pub imei: String,
}

/// Battery state from `dumpsys battery`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BatteryInfo {
    pub level: u8,
    pub scale: u8,
    /// One of AC / USB / Wireless / None.
    pub power_source: String,
    pub status: BatteryStatus,
    pub health: String,
    /// Temperature in degrees Celsius (dumpsys reports tenths of a degree).
    pub temperature_c: f32,
    /// Voltage in millivolts.
    pub voltage_mv: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BatteryStatus {
    #[default]
    Unknown,
    Charging,
    Discharging,
    NotCharging,
    Full,
}

/// A single installed package, as listed by `pm list packages`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageRef {
    pub name: String,
    /// Whether `pm` flagged this as a system package.
    pub system: bool,
}

/// Detailed package info from `dumpsys package <name>`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageDetail {
    pub name: String,
    pub version_name: String,
    pub version_code: u64,
    pub min_sdk: u32,
    pub target_sdk: u32,
    pub permissions: Vec<Permission>,
    pub activities: Vec<Component>,
    pub services: Vec<Component>,
    pub receivers: Vec<Component>,
    /// Content providers almost never appear here: they're invoked by
    /// authority URI, not intent, so they have no entry in the resolver
    /// tables this list is built from. Expect this to usually be empty.
    pub providers: Vec<Component>,
    /// Name of the activity with both `MAIN` action and `LAUNCHER` category,
    /// if one was found — the icon a user taps to open the app.
    pub launcher_activity: Option<String>,
    /// Full unparsed `dumpsys package` text, kept alongside the structured
    /// fields so the UI can show it directly — useful both as a fallback
    /// when a field isn't parsed yet and for capturing real-device fixtures.
    pub raw: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Permission {
    pub name: String,
    pub granted: bool,
    pub protection_level: ProtectionLevel,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProtectionLevel {
    #[default]
    Normal,
    Dangerous,
    Signature,
    Unknown,
}

/// A declared component (activity/service/receiver/provider), as seen in
/// `dumpsys package`'s intent-resolver tables.
///
/// Note there's no `exported` field: the real Android build this was tested
/// against never prints an explicit `exported=` value anywhere in
/// `dumpsys package` or `pm dump` output for any component. The only proxy
/// available from this text is "has a declared intent-filter" (which is what
/// landing in a resolver table means), and that's not equivalent to
/// `exported="true"` — a developer can have an intent-filter and still set
/// `exported="false"` explicitly, and that override isn't visible here. The
/// only fully accurate way to get the flag is parsing AndroidManifest.xml
/// out of the APK directly, which this parser doesn't do.
/// Content of a single file read from a debuggable app's private data
/// directory via `run-as <pkg> cat <path>`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageFileContent {
    /// Path relative to the app's data directory, e.g. `shared_prefs/app.xml`.
    pub path: String,
    /// File content, lossily decoded as UTF-8 and capped at a byte limit.
    pub content: String,
    /// True if `content` was cut short because the file exceeded the cap.
    pub truncated: bool,
    /// Size of the content actually read, in bytes (before any truncation).
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Component {
    pub name: String,
    /// Intent actions this component declares a filter for, e.g.
    /// `android.intent.action.BOOT_COMPLETED`.
    pub intent_actions: Vec<String>,
    /// Permission required to invoke this component, if the resolver table
    /// listed one (e.g. `android.permission.BIND_QUICK_SETTINGS_TILE`).
    pub permission: Option<String>,
}

/// One process row from `top -b -n1`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub user: String,
    pub cpu_percent: f32,
    pub mem_percent: f32,
    pub name: String,
}

/// One log line from `logcat`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogEntry {
    pub timestamp: String,
    pub pid: u32,
    pub tid: u32,
    pub priority: LogPriority,
    pub tag: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogPriority {
    Verbose,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
    Fatal,
}

impl LogPriority {
    /// Map logcat's single-letter priority code (V/D/I/W/E/F) to the enum.
    pub fn from_code(c: char) -> Self {
        match c {
            'V' => Self::Verbose,
            'D' => Self::Debug,
            'I' => Self::Info,
            'W' => Self::Warn,
            'E' => Self::Error,
            'F' => Self::Fatal,
            _ => Self::Info,
        }
    }
}
