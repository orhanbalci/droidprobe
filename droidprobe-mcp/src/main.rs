//! MCP server front-end.
//!
//! Exposes the same `droidprobe-core` engine as MCP **tools** so an agent (e.g.
//! Claude) can inspect a connected Android device and reason about misbehaving
//! apps. Each tool is a thin wrapper that calls `engine.run(<command id>, …)`
//! and returns the JSON output as tool content.
//!
//! Safety: the engine is constructed with `read_only: true`, so even if a
//! mutating command is registered it cannot be invoked through this surface.
//!
//! Built against the official `rmcp` SDK using the `#[tool_router]` / `#[tool]`
//! macros. The exact attribute surface of rmcp shifts between releases; pin the
//! version in the workspace manifest and adjust imports if you bump it.

use std::sync::Arc;

use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde::Deserialize;

use droidprobe_core::{
    engine::{Engine, EngineConfig},
    registry::Registry,
    AdbTransport,
};

/// Args accepting an optional device serial, shared by no-arg device tools.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SerialArgs {
    /// Target device serial. Omit when only one device is connected.
    #[serde(default)]
    pub serial: Option<String>,
}

/// Args for package detail lookup.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct PackageArgs {
    /// The package name, e.g. `com.example.app`.
    pub package: String,
    #[serde(default)]
    pub serial: Option<String>,
}

/// Args for a logcat snapshot.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LogcatArgs {
    /// Maximum number of most-recent lines to return.
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub serial: Option<String>,
}

#[derive(Clone)]
pub struct AndroidInspector {
    engine: Engine,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl AndroidInspector {
    pub fn new(engine: Engine) -> Self {
        Self {
            engine,
            tool_router: Self::tool_router(),
        }
    }

    /// Run a core command and wrap its JSON output as MCP tool content.
    async fn call(
        &self,
        id: &str,
        serial: Option<&str>,
        args: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        match self.engine.run(id, serial, args).await {
            Ok(json) => {
                let text = serde_json::to_string_pretty(&json).unwrap_or_else(|_| json.to_string());
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }

    #[tool(description = "Get device model, manufacturer, Android version and ABI")]
    async fn get_device_info(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<SerialArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call(
            "device.info",
            args.serial.as_deref(),
            serde_json::Value::Null,
        )
        .await
    }

    #[tool(description = "Get CPU core count and hardware/chipset name")]
    async fn get_cpu_info(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<SerialArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call(
            "device.cpu",
            args.serial.as_deref(),
            serde_json::Value::Null,
        )
        .await
    }

    #[tool(description = "Get screen resolution and density")]
    async fn get_screen_info(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<SerialArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call(
            "device.screen",
            args.serial.as_deref(),
            serde_json::Value::Null,
        )
        .await
    }

    #[tool(description = "Get total and available RAM")]
    async fn get_memory_info(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<SerialArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call(
            "device.memory",
            args.serial.as_deref(),
            serde_json::Value::Null,
        )
        .await
    }

    #[tool(description = "Get mounted filesystem sizes and free space")]
    async fn get_storage_info(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<SerialArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call(
            "device.storage",
            args.serial.as_deref(),
            serde_json::Value::Null,
        )
        .await
    }

    #[tool(
        description = "Get the device IMEI, if readable without a privileged permission (often empty on Android 10+)"
    )]
    async fn get_imei(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<SerialArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call(
            "device.imei",
            args.serial.as_deref(),
            serde_json::Value::Null,
        )
        .await
    }

    #[tool(description = "Get battery level, charging state, temperature and voltage")]
    async fn get_battery_status(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<SerialArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call(
            "battery.status",
            args.serial.as_deref(),
            serde_json::Value::Null,
        )
        .await
    }

    #[tool(description = "List installed third-party packages on the device")]
    async fn list_packages(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<SerialArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call(
            "package.list",
            args.serial.as_deref(),
            serde_json::Value::Null,
        )
        .await
    }

    #[tool(
        description = "Get permissions and components (activities, services, receivers) for a package"
    )]
    async fn get_package_details(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<PackageArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.call(
            "package.detail",
            args.serial.as_deref(),
            serde_json::Value::String(args.package),
        )
        .await
    }

    #[tool(
        description = "Get a recent snapshot of logcat output, useful for diagnosing crashes/ANRs"
    )]
    async fn get_logcat(
        &self,
        rmcp::handler::server::wrapper::Parameters(args): rmcp::handler::server::wrapper::Parameters<LogcatArgs>,
    ) -> Result<CallToolResult, McpError> {
        let payload = serde_json::json!({ "limit": args.limit });
        self.call("log.snapshot", args.serial.as_deref(), payload)
            .await
    }
}

#[tool_handler]
impl ServerHandler for AndroidInspector {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(
                "Inspect a connected Android device over ADB. Tools are read-only: \
                 they report device identity/hardware (model, CPU, screen, RAM, \
                 storage, IMEI), battery, installed packages, package \
                 permissions/components, and logcat snapshots. Use get_logcat plus \
                 get_package_details to investigate why an app is misbehaving."
                    .into(),
            ),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // MCP uses stdout for protocol traffic; logs must go to stderr.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let transport = Arc::new(AdbTransport::default());
    let registry = Arc::new(Registry::with_builtins());
    let engine = Engine::new(
        registry,
        transport,
        EngineConfig {
            read_only: true,
            default_serial: None,
        },
    );

    tracing::info!("starting droidprobe-mcp server on stdio");
    let service = AndroidInspector::new(engine).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
