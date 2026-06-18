//! The [`Command`] trait and its object-safe sibling [`DynCommand`].
//!
//! `Command` is the strongly-typed contract (it has an associated `Output`).
//! `DynCommand` is the type-erased form the registry stores in a map: it always
//! produces JSON, which is exactly what both the MCP server and a generic TUI
//! data cache want. A blanket impl bridges any `Command` into a `DynCommand`,
//! so authors only implement the typed trait.

use async_trait::async_trait;
use serde::Serialize;

use crate::error::{CommandError, CommandResult};
use crate::transport::Transport;

/// JSON value alias used as the universal output of type-erased commands.
pub type Json = serde_json::Value;

/// Broad grouping used by the TUI to organize commands into tabs/menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Device,
    Battery,
    Memory,
    Cpu,
    Process,
    Package,
    Network,
    Log,
    Other,
}

/// Static, cheap-to-clone descriptive metadata about a command.
#[derive(Debug, Clone, Serialize)]
pub struct CommandMeta {
    /// Stable unique identifier, e.g. `"battery.status"`. Used as the registry
    /// key and (for MCP) the tool name.
    pub id: &'static str,
    /// One-line human/agent-facing description.
    pub description: &'static str,
    pub category: Category,
    /// True if running this changes device state (install, reboot, set prop).
    /// The core registry refuses these in read-only mode; the MCP server hides
    /// them unless explicitly opted in.
    pub mutating: bool,
}

/// A typed, runnable command. Implement this to add a command.
///
/// `Args` lets a command take parameters (e.g. a package name). For commands
/// that take none, use `()`.
#[async_trait]
pub trait Command: Send + Sync + 'static {
    /// Parameters required to run the command.
    type Args: Send + Sync;
    /// Structured, serializable result.
    type Output: Serialize + Send;

    /// Descriptive metadata; must be constant for a given type.
    fn meta(&self) -> CommandMeta;

    /// Execute against the device via `transport`.
    async fn run(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        args: Self::Args,
    ) -> CommandResult<Self::Output>;
}

/// Object-safe, type-erased command stored by the registry.
///
/// Arguments arrive as a JSON value (so a heterogeneous map of commands can be
/// driven uniformly), and output is JSON. Authors normally don't implement this
/// directly — see [`TypedDyn`], which adapts any [`Command`] whose `Args`
/// implement `DeserializeOwned`.
#[async_trait]
pub trait DynCommand: Send + Sync {
    fn meta(&self) -> CommandMeta;

    /// Run with JSON args, returning JSON output. `args` may be `Json::Null`
    /// for argument-less commands.
    async fn run_json(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        args: Json,
    ) -> CommandResult<Json>;
}

/// Adapter that turns a typed [`Command`] into a [`DynCommand`] by
/// deserializing JSON args and serializing the typed output back to JSON.
pub struct TypedDyn<C>(pub C);

#[async_trait]
impl<C> DynCommand for TypedDyn<C>
where
    C: Command,
    C::Args: serde::de::DeserializeOwned,
{
    fn meta(&self) -> CommandMeta {
        self.0.meta()
    }

    async fn run_json(
        &self,
        transport: &dyn Transport,
        serial: Option<&str>,
        args: Json,
    ) -> CommandResult<Json> {
        // `null` deserializes into `()` and into `Option<_>::None`, so
        // argument-less and optional-arg commands "just work".
        let typed: C::Args = serde_json::from_value(args)
            .map_err(|e| CommandError::InvalidArgument(e.to_string()))?;
        let out = self.0.run(transport, serial, typed).await?;
        Ok(serde_json::to_value(out)?)
    }
}
