//! The command registry: a name-keyed map of type-erased commands.
//!
//! Built-ins are loaded via [`Registry::with_builtins`]. Third-party authors
//! add their own with [`Registry::register`], which is how the "other authors
//! can write their own commands" requirement is satisfied — they hand us a
//! `Box<dyn DynCommand>` (usually `TypedDyn(TheirCommand)`).

use std::collections::BTreeMap;

use droidprobe_command::{CommandMeta, DynCommand};

#[derive(Default)]
pub struct Registry {
    commands: BTreeMap<&'static str, Box<dyn DynCommand>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a registry pre-loaded with all built-in commands.
    pub fn with_builtins() -> Self {
        let mut reg = Self::new();
        for cmd in droidprobe_command::catalog::builtins() {
            reg.register(cmd);
        }
        reg
    }

    /// Register a command. If a command with the same id exists it is replaced,
    /// allowing authors to override a built-in deliberately.
    pub fn register(&mut self, cmd: Box<dyn DynCommand>) -> &mut Self {
        let id = cmd.meta().id;
        if self.commands.insert(id, cmd).is_some() {
            tracing::warn!(command = id, "replaced an existing command registration");
        }
        self
    }

    pub fn get(&self, id: &str) -> Option<&dyn DynCommand> {
        self.commands.get(id).map(|b| b.as_ref())
    }

    pub fn contains(&self, id: &str) -> bool {
        self.commands.contains_key(id)
    }

    /// Metadata for every registered command, sorted by id (BTreeMap order).
    pub fn list(&self) -> Vec<CommandMeta> {
        self.commands.values().map(|c| c.meta()).collect()
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
