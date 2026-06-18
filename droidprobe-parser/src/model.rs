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
    pub providers: Vec<Component>,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Component {
    pub name: String,
    /// Exported components are reachable from outside the app — a security flag
    /// worth surfacing prominently.
    pub exported: bool,
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
