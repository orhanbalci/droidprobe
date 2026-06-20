//! Built-in command catalog.
//!
//! Each command is a small zero-sized type implementing [`Command`]. This file
//! covers the core read-only inspection set; extending toward the full ATK
//! surface means adding more types here (memory, network, cpu, install/uninstall
//! as `mutating`, etc.) following the same pattern.

use async_trait::async_trait;

use droidprobe_parser::model::{
    BatteryInfo, CpuInfo, DeviceInfo, ImeiInfo, LogEntry, MemoryInfo, PackageDetail, PackageRef,
    ScreenInfo, StorageEntry,
};
use droidprobe_parser::parsers::{
    battery::BatteryParser, cpuinfo::CpuInfoParser, getprop::GetpropParser, imei::ImeiInfoParser,
    logcat::LogcatParser, meminfo::MemInfoParser, package_dump::PackageDumpParser,
    packages::PackageListParser, screen::ScreenInfoParser, storage::StorageParser,
};
use droidprobe_parser::Parse;

use crate::command::{Category, Command, CommandMeta, DynCommand, TypedDyn};
use crate::error::CommandResult;
use crate::transport::Transport;

/// Device identity via `getprop`.
pub struct DeviceInfoCmd;

#[async_trait]
impl Command for DeviceInfoCmd {
    type Args = ();
    type Output = DeviceInfo;

    fn meta(&self) -> CommandMeta {
        CommandMeta {
            id: "device.info",
            description: "Device model, manufacturer, Android version and ABI",
            category: Category::Device,
            mutating: false,
        }
    }

    async fn run(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        _args: (),
    ) -> CommandResult<DeviceInfo> {
        let raw = transport.shell(serial, "getprop").await?;
        Ok(GetpropParser::parse(&raw)?)
    }
}

/// Battery state via `dumpsys battery`.
pub struct BatteryCmd;

#[async_trait]
impl Command for BatteryCmd {
    type Args = ();
    type Output = BatteryInfo;

    fn meta(&self) -> CommandMeta {
        CommandMeta {
            id: "battery.status",
            description: "Battery level, charging state, temperature and voltage",
            category: Category::Battery,
            mutating: false,
        }
    }

    async fn run(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        _args: (),
    ) -> CommandResult<BatteryInfo> {
        let raw = transport.shell(serial, "dumpsys battery").await?;
        Ok(BatteryParser::parse(&raw)?)
    }
}

/// Installed third-party packages via `pm list packages -3`.
pub struct ListPackagesCmd;

#[async_trait]
impl Command for ListPackagesCmd {
    type Args = ();
    type Output = Vec<PackageRef>;

    fn meta(&self) -> CommandMeta {
        CommandMeta {
            id: "package.list",
            description: "List installed third-party packages",
            category: Category::Package,
            mutating: false,
        }
    }

    async fn run(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        _args: (),
    ) -> CommandResult<Vec<PackageRef>> {
        let raw = transport.shell(serial, "pm list packages -3").await?;
        Ok(PackageListParser::parse_with_system(&raw, false)?)
    }
}

/// Detailed package info via `dumpsys package <name>`.
pub struct PackageDetailCmd;

#[async_trait]
impl Command for PackageDetailCmd {
    type Args = String; // package name
    type Output = PackageDetail;

    fn meta(&self) -> CommandMeta {
        CommandMeta {
            id: "package.detail",
            description: "Permissions and components for a specific package",
            category: Category::Package,
            mutating: false,
        }
    }

    async fn run(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        pkg: String,
    ) -> CommandResult<PackageDetail> {
        let raw = transport
            .shell(serial, &format!("dumpsys package {pkg}"))
            .await?;
        Ok(PackageDumpParser::parse_for(&pkg, &raw)?)
    }
}

/// CPU summary via `cat /proc/cpuinfo`.
pub struct CpuInfoCmd;

#[async_trait]
impl Command for CpuInfoCmd {
    type Args = ();
    type Output = CpuInfo;

    fn meta(&self) -> CommandMeta {
        CommandMeta {
            id: "device.cpu",
            description: "CPU core count and hardware/chipset name",
            category: Category::Device,
            mutating: false,
        }
    }

    async fn run(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        _args: (),
    ) -> CommandResult<CpuInfo> {
        let raw = transport.shell(serial, "cat /proc/cpuinfo").await?;
        let mut info = CpuInfoParser::parse(&raw)?;

        // Many modern arm64 kernels drop the `Hardware` line from
        // /proc/cpuinfo entirely; fall back to the board/chipset prop.
        if info.hardware.is_empty() {
            for prop in ["ro.hardware", "ro.board.platform", "ro.product.board"] {
                let val = transport.shell(serial, &format!("getprop {prop}")).await?;
                let val = val.trim();
                if !val.is_empty() {
                    info.hardware = val.to_string();
                    break;
                }
            }
        }

        Ok(info)
    }
}

/// Screen geometry via `wm size` + `wm density`.
pub struct ScreenInfoCmd;

#[async_trait]
impl Command for ScreenInfoCmd {
    type Args = ();
    type Output = ScreenInfo;

    fn meta(&self) -> CommandMeta {
        CommandMeta {
            id: "device.screen",
            description: "Screen resolution and density",
            category: Category::Device,
            mutating: false,
        }
    }

    async fn run(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        _args: (),
    ) -> CommandResult<ScreenInfo> {
        let size_raw = transport.shell(serial, "wm size").await?;
        let density_raw = transport.shell(serial, "wm density").await?;
        Ok(ScreenInfoParser::parse_combined(&size_raw, &density_raw)?)
    }
}

/// RAM totals via `cat /proc/meminfo`.
pub struct MemInfoCmd;

#[async_trait]
impl Command for MemInfoCmd {
    type Args = ();
    type Output = MemoryInfo;

    fn meta(&self) -> CommandMeta {
        CommandMeta {
            id: "device.memory",
            description: "Total and available RAM",
            category: Category::Device,
            mutating: false,
        }
    }

    async fn run(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        _args: (),
    ) -> CommandResult<MemoryInfo> {
        let raw = transport.shell(serial, "cat /proc/meminfo").await?;
        Ok(MemInfoParser::parse(&raw)?)
    }
}

/// Mounted filesystem usage via `df`.
pub struct StorageInfoCmd;

#[async_trait]
impl Command for StorageInfoCmd {
    type Args = ();
    type Output = Vec<StorageEntry>;

    fn meta(&self) -> CommandMeta {
        CommandMeta {
            id: "device.storage",
            description: "Mounted filesystem sizes and free space",
            category: Category::Device,
            mutating: false,
        }
    }

    async fn run(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        _args: (),
    ) -> CommandResult<Vec<StorageEntry>> {
        let raw = transport.shell(serial, "df").await?;
        Ok(StorageParser::parse(&raw)?)
    }
}

/// IMEI via `dumpsys iphonesubinfo`. Frequently empty on Android 10+ — that's
/// a permission gap on the device, not a command failure.
pub struct ImeiInfoCmd;

#[async_trait]
impl Command for ImeiInfoCmd {
    type Args = ();
    type Output = ImeiInfo;

    fn meta(&self) -> CommandMeta {
        CommandMeta {
            id: "device.imei",
            description: "Device IMEI, if readable without a privileged permission",
            category: Category::Device,
            mutating: false,
        }
    }

    async fn run(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        _args: (),
    ) -> CommandResult<ImeiInfo> {
        let raw = transport.shell(serial, "dumpsys iphonesubinfo").await?;
        Ok(ImeiInfoParser::parse(&raw)?)
    }
}

/// A bounded logcat snapshot via `logcat -d -v threadtime`.
pub struct LogcatSnapshotCmd;

/// Optional filters for a logcat snapshot.
#[derive(Debug, Default, serde::Deserialize)]
pub struct LogcatArgs {
    /// Limit to this many most-recent lines (applied after parsing).
    pub limit: Option<usize>,
}

#[async_trait]
impl Command for LogcatSnapshotCmd {
    type Args = LogcatArgs;
    type Output = Vec<LogEntry>;

    fn meta(&self) -> CommandMeta {
        CommandMeta {
            id: "log.snapshot",
            description: "Recent logcat lines (non-streaming snapshot)",
            category: Category::Log,
            mutating: false,
        }
    }

    async fn run(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        args: LogcatArgs,
    ) -> CommandResult<Vec<LogEntry>> {
        let raw = transport.shell(serial, "logcat -d -v threadtime").await?;
        let mut entries = LogcatParser::parse(&raw)?;
        if let Some(limit) = args.limit {
            let start = entries.len().saturating_sub(limit);
            entries = entries.split_off(start);
        }
        Ok(entries)
    }
}

/// Return all built-in commands, type-erased and ready to register.
pub fn builtins() -> Vec<Box<dyn DynCommand>> {
    vec![
        Box::new(TypedDyn(DeviceInfoCmd)),
        Box::new(TypedDyn(BatteryCmd)),
        Box::new(TypedDyn(ListPackagesCmd)),
        Box::new(TypedDyn(PackageDetailCmd)),
        Box::new(TypedDyn(CpuInfoCmd)),
        Box::new(TypedDyn(ScreenInfoCmd)),
        Box::new(TypedDyn(MemInfoCmd)),
        Box::new(TypedDyn(StorageInfoCmd)),
        Box::new(TypedDyn(ImeiInfoCmd)),
        Box::new(TypedDyn(LogcatSnapshotCmd)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MockTransport;

    #[tokio::test]
    async fn battery_command_end_to_end() {
        let t = MockTransport::default().with(
            "dumpsys battery",
            "  status: 2\n  level: 90\n  scale: 100\n  voltage: 4000\n  temperature: 250\n  USB powered: true",
        );
        let out = BatteryCmd
            .run(&t, None, ())
            .await
            .expect("command should succeed");
        assert_eq!(out.level, 90);
        assert_eq!(out.power_source, "USB");
    }

    #[tokio::test]
    async fn dyn_command_roundtrips_json() {
        let t = MockTransport::default().with("pm list packages", "package:com.a\npackage:com.b");
        let cmd = TypedDyn(ListPackagesCmd);
        let out = cmd
            .run_json(&t, None, serde_json::Value::Null)
            .await
            .unwrap();
        assert!(out.is_array());
        assert_eq!(out.as_array().unwrap().len(), 2);
    }
}
