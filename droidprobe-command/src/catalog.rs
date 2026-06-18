//! Built-in command catalog.
//!
//! Each command is a small zero-sized type implementing [`Command`]. This file
//! covers the core read-only inspection set; extending toward the full ATK
//! surface means adding more types here (memory, network, cpu, install/uninstall
//! as `mutating`, etc.) following the same pattern.

use async_trait::async_trait;

use droidprobe_parser::model::{BatteryInfo, DeviceInfo, LogEntry, PackageDetail, PackageRef};
use droidprobe_parser::parsers::{
    battery::BatteryParser, getprop::GetpropParser, logcat::LogcatParser,
    packages::PackageListParser,
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
///
/// NOTE: the `dumpsys package` parser is intentionally left as a stub in the
/// parser crate's roadmap — see implementation plan. This command shows the
/// arg-taking pattern; wire it to a real parser once written.
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
        let _raw = transport
            .shell(serial, &format!("dumpsys package {pkg}"))
            .await?;
        // TODO: replace with PackageDumpParser::parse(&_raw) once implemented.
        Ok(PackageDetail {
            name: pkg,
            ..Default::default()
        })
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
        let t = MockTransport::default()
            .with("pm list packages", "package:com.a\npackage:com.b");
        let cmd = TypedDyn(ListPackagesCmd);
        let out = cmd
            .run_json(&t, None, serde_json::Value::Null)
            .await
            .unwrap();
        assert!(out.is_array());
        assert_eq!(out.as_array().unwrap().len(), 2);
    }
}
