//! The execution engine ties a [`Registry`] to a [`Transport`] and runs
//! commands by id, returning JSON. It enforces the read-only safety boundary
//! centrally so neither front-end has to.

use std::sync::Arc;

use droidprobe_command::{CommandError, CommandResult, DynCommand, Json, Transport};

use crate::registry::Registry;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// When true, any command whose `meta().mutating` is true is refused.
    /// Defaults to true — safety first, especially for the MCP server.
    pub read_only: bool,
    /// Default device serial when a caller doesn't specify one.
    pub default_serial: Option<String>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            read_only: true,
            default_serial: None,
        }
    }
}

/// Owns the registry and transport; the single entry point for "run command X".
#[derive(Clone)]
pub struct Engine {
    registry: Arc<Registry>,
    transport: Arc<dyn Transport>,
    config: EngineConfig,
}

impl Engine {
    pub fn new(
        registry: Arc<Registry>,
        transport: Arc<dyn Transport>,
        config: EngineConfig,
    ) -> Self {
        Self {
            registry,
            transport,
            config,
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Run the command `id` with `args`, returning its JSON output.
    ///
    /// `serial` overrides the engine default for this one call. Mutating
    /// commands are refused when `read_only` is set.
    pub async fn run(
        &self,
        id: &str,
        serial: Option<&str>,
        args: Json,
    ) -> CommandResult<Json> {
        let cmd: &dyn DynCommand = self
            .registry
            .get(id)
            .ok_or_else(|| CommandError::InvalidArgument(format!("unknown command `{id}`")))?;

        if self.config.read_only && cmd.meta().mutating {
            return Err(CommandError::ReadOnlyViolation(id.to_string()));
        }

        let serial = serial.or(self.config.default_serial.as_deref());
        tracing::debug!(command = id, ?serial, "executing command");
        cmd.run_json(self.transport.as_ref(), serial, args).await
    }

    /// Convenience: run a command that takes no arguments.
    pub async fn run_no_args(&self, id: &str, serial: Option<&str>) -> CommandResult<Json> {
        self.run(id, serial, Json::Null).await
    }
}
