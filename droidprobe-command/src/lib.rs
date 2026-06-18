//! # droidprobe-command
//!
//! Runnable commands against an Android device. A [`Command`] knows three things:
//! how to *invoke* itself over a [`transport::Transport`], how to *decode* the
//! resulting text (delegating to `droidprobe-parser`), and enough metadata
//! (`id`, `description`, `category`, `mutating`) for the core registry, the TUI,
//! and the MCP server to reason about it.
//!
//! Third-party authors implement [`Command`] (or the object-safe
//! [`DynCommand`]) to add their own commands, then register them with
//! `droidprobe-core`.

pub mod catalog;
pub mod command;
pub mod error;
pub mod transport;

pub use command::{Category, Command, CommandMeta, DynCommand, Json};
pub use error::{CommandError, CommandResult};
pub use transport::{AdbTransport, MockTransport, Transport};
