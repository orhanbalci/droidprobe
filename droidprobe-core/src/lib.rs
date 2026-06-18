//! # droidprobe-core
//!
//! The orchestration layer. It owns:
//!
//! - a [`registry::Registry`] of [`DynCommand`]s (built-ins plus any registered
//!   by third-party authors),
//! - an [`Engine`] that runs a command by id, enforcing read-only mode,
//! - a [`poller::Poller`] that re-runs selected commands on intervals and
//!   publishes results over a broadcast channel.
//!
//! Both front-ends (`droidprobe-tui`, `droidprobe-mcp`) depend only on this crate.

pub mod engine;
pub mod poller;
pub mod registry;

pub use engine::{Engine, EngineConfig};
pub use poller::{PollHandle, PollUpdate, Poller};
pub use registry::Registry;

// Re-export the command surface so front-ends need only depend on core.
pub use droidprobe_command::{
    catalog, AdbTransport, Category, Command, CommandError, CommandMeta, CommandResult, DynCommand,
    Json, MockTransport, Transport,
};
