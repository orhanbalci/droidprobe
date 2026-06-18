//! Application state. Holds the current view and the most recent JSON snapshot
//! for each command id. Keeping snapshots as raw `serde_json::Value` means the
//! UI layer can render whatever commands happen to be polled without the state
//! struct needing to know every concrete output type.

use std::collections::HashMap;

use droidprobe_core::poller::PollUpdate;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Overview,
    Packages,
    Logs,
}

impl View {
    pub fn title(&self) -> &'static str {
        match self {
            View::Overview => "Overview",
            View::Packages => "Packages",
            View::Logs => "Logs",
        }
    }
}

pub struct App {
    pub running: bool,
    pub view: View,
    /// Latest output per command id, plus any error string.
    pub snapshots: HashMap<String, Result<Value, String>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            view: View::Overview,
            snapshots: HashMap::new(),
        }
    }

    pub fn next_view(&mut self) {
        self.view = match self.view {
            View::Overview => View::Packages,
            View::Packages => View::Logs,
            View::Logs => View::Overview,
        };
    }

    pub fn apply_update(&mut self, update: PollUpdate) {
        self.snapshots.insert(update.command_id, update.result);
    }

    /// Convenience accessor for a successfully-fetched snapshot.
    pub fn snapshot_ok(&self, id: &str) -> Option<&Value> {
        self.snapshots.get(id).and_then(|r| r.as_ref().ok())
    }
}
