//! The transport carries shell strings to a device and returns raw stdout.
//!
//! Commands are written against the [`Transport`] trait, not against
//! `adb_client` directly. That keeps commands testable with [`MockTransport`]
//! and leaves room to swap in a direct-USB backend later without touching
//! command code.

use async_trait::async_trait;

use crate::error::{CommandError, CommandResult};

/// Sends shell command lines to a device and returns their stdout.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Run `shell_cmd` (e.g. `"dumpsys battery"`) on the device identified by
    /// `serial` (or the sole connected device when `None`) and return stdout.
    async fn shell(&self, serial: Option<&str>, shell_cmd: &str) -> CommandResult<String>;

    /// List currently connected device serials.
    async fn list_devices(&self) -> CommandResult<Vec<String>>;
}

/// Real transport backed by an `adb` server over the `adb_client` crate.
///
/// This adapter is intentionally thin. Note that `adb_client`'s server client
/// is synchronous, so calls are dispatched onto a blocking thread via
/// `tokio::task::spawn_blocking` to avoid stalling the async runtime.
pub struct AdbTransport {
    /// `127.0.0.1:5037` by default — the local adb server.
    pub server_addr: std::net::SocketAddrV4,
}

impl Default for AdbTransport {
    fn default() -> Self {
        Self {
            server_addr: std::net::SocketAddrV4::new(
                std::net::Ipv4Addr::new(127, 0, 0, 1),
                5037,
            ),
        }
    }
}

#[async_trait]
impl Transport for AdbTransport {
    async fn shell(&self, serial: Option<&str>, shell_cmd: &str) -> CommandResult<String> {
        let addr = self.server_addr;
        let serial = serial.map(|s| s.to_string());
        let cmd = shell_cmd.to_string();

        // adb_client's ADBServer/ADBServerDevice APIs are blocking; run them off
        // the async executor. The exact call surface is pinned in one place so
        // an adb_client version bump only touches this function.
        tokio::task::spawn_blocking(move || -> CommandResult<String> {
            use adb_client::{ADBDeviceExt, ADBServer};

            let mut server = ADBServer::new(addr);
            let mut device = match serial.as_deref() {
                Some(s) => server
                    .get_device_by_name(s)
                    .map_err(|e| CommandError::Transport(e.to_string()))?,
                None => server
                    .get_device()
                    .map_err(|e| CommandError::Transport(e.to_string()))?,
            };

            let parts: Vec<&str> = cmd.split_whitespace().collect();
            let mut out: Vec<u8> = Vec::new();
            device
                .shell_command(&parts, &mut out)
                .map_err(|e| CommandError::Transport(e.to_string()))?;

            Ok(String::from_utf8_lossy(&out).into_owned())
        })
        .await
        .map_err(|e| CommandError::Transport(format!("join error: {e}")))?
    }

    async fn list_devices(&self) -> CommandResult<Vec<String>> {
        let addr = self.server_addr;
        tokio::task::spawn_blocking(move || -> CommandResult<Vec<String>> {
            use adb_client::ADBServer;
            let mut server = ADBServer::new(addr);
            let devices = server
                .devices()
                .map_err(|e| CommandError::Transport(e.to_string()))?;
            Ok(devices.into_iter().map(|d| d.identifier).collect())
        })
        .await
        .map_err(|e| CommandError::Transport(format!("join error: {e}")))?
    }
}

/// In-memory transport for unit tests: maps a shell command substring to a
/// canned response. Lets commands be tested with zero hardware.
#[derive(Default)]
pub struct MockTransport {
    /// (substring-to-match, response) pairs, checked in order.
    pub responses: Vec<(String, String)>,
    pub devices: Vec<String>,
}

impl MockTransport {
    pub fn with(mut self, contains: &str, response: &str) -> Self {
        self.responses
            .push((contains.to_string(), response.to_string()));
        self
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn shell(&self, _serial: Option<&str>, shell_cmd: &str) -> CommandResult<String> {
        self.responses
            .iter()
            .find(|(needle, _)| shell_cmd.contains(needle.as_str()))
            .map(|(_, resp)| resp.clone())
            .ok_or_else(|| CommandError::Transport(format!("no mock for `{shell_cmd}`")))
    }

    async fn list_devices(&self) -> CommandResult<Vec<String>> {
        Ok(self.devices.clone())
    }
}
