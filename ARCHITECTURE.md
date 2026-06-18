# droidprobe — Architecture & Implementation Plan

A modular Rust toolkit for inspecting an Android device over ADB (USB debugging),
with two front-ends — a terminal UI for humans and an MCP server for agents —
sharing one data/command core.

## Design goals

1. **Separation of concerns.** Parsing, command execution, orchestration, and
   presentation each live in their own crate and can be tested in isolation.
2. **Extensibility.** Third-party authors can add commands without forking, by
   implementing a trait and registering with the core.
3. **One core, many front-ends.** The TUI and the MCP server are thin shells
   over the same engine, so a feature added to the core is available to both.
4. **Safety by default.** Mutating operations are gated behind an explicit
   read-only switch that defaults to on — critical for the agent-facing surface.

## Crate graph

```
            droidprobe-parser   (pure text -> structs, no I/O)
                  ▲
                  │
            droidprobe-command  (Command trait, Transport, built-in catalog)
                  ▲
                  │
            droidprobe-core     (Registry + Engine + Poller)
                 ▲   ▲
        ┌────────┘   └────────┐
   droidprobe-tui            droidprobe-mcp
   (ratatui binary)      (rmcp binary)
```

Dependencies point strictly upward; there are no cycles. A front-end never
depends on `droidprobe-parser` or `droidprobe-command` directly — `droidprobe-core`
re-exports what they need.

## Crate responsibilities

### droidprobe-parser
Transforms raw command output into structured, serializable types. Zero I/O,
zero command execution — every parser is a pure `&str -> Result<T>` function
behind the `Parse` trait, tested against captured fixture strings.

- `model` — canonical types (`DeviceInfo`, `BatteryInfo`, `PackageDetail`,
  `ProcessInfo`, `LogEntry`, …), all `Serialize`/`Deserialize`.
- `parsers` — one submodule per command family.

### droidprobe-command
Runnable commands plus the device transport.

- `Transport` trait abstracts the device link. `AdbTransport` wraps `adb_client`
  (dispatching its blocking calls via `spawn_blocking`); `MockTransport` feeds
  canned strings for tests.
- `Command` is the typed author-facing trait: associated `Args` and `Output`,
  plus `meta()` describing id/category/`mutating`.
- `DynCommand` is the object-safe, JSON-in/JSON-out form stored by the registry;
  `TypedDyn<C>` adapts any `Command` into it automatically.
- `catalog` holds the built-in command set and a `builtins()` constructor.

This is where the bulk of the ATK-equivalent surface accrues over time: each ATK
action becomes a `Command` whose `meta().mutating` flag classifies it.

### droidprobe-core
Orchestration shared by both front-ends.

- `Registry` — id-keyed map of `DynCommand`s. `with_builtins()` loads the
  catalog; `register()` accepts third-party commands (and can override built-ins).
- `Engine` — runs a command by id against a transport, enforcing read-only mode
  and resolving the default serial.
- `Poller` — spawns a tokio task per recurring job; each broadcasts `PollUpdate`s
  that the TUI subscribes to. Fires once immediately, then on the interval.

### droidprobe-tui
`ratatui` + `crossterm` binary. A `tokio::select!` loop merges crossterm's async
`EventStream` (keys) with the poller's broadcast channel (data). State lives in
`App` as per-command JSON snapshots; `ui::draw` is a pure render of that state.

### droidprobe-mcp
`rmcp` binary over stdio. Each MCP tool wraps `engine.run(<id>, …)`. The engine
is read-only, so the agent surface is inspection-only. Tools: `get_device_info`,
`get_battery_status`, `list_packages`, `get_package_details`, `get_logcat`.

## Extension model (third-party commands)

```rust
use droidprobe_command::{Command, CommandMeta, Category, TypedDyn};

struct WifiInfoCmd;

#[async_trait::async_trait]
impl Command for WifiInfoCmd {
    type Args = ();
    type Output = MyWifiInfo;            // any Serialize type
    fn meta(&self) -> CommandMeta { /* id="network.wifi", mutating:false … */ }
    async fn run(&self, t: &dyn Transport, serial: Option<&str>, _: ())
        -> CommandResult<MyWifiInfo> { /* shell + parse */ }
}

// Register into the core:
let mut registry = Registry::with_builtins();
registry.register(Box::new(TypedDyn(WifiInfoCmd)));
```

Because registration takes `Box<dyn DynCommand>`, authors can ship commands from
their own crate and downstream users compose them at startup. (A `dlopen`-style
plugin system is intentionally out of scope — static registration keeps the type
safety and is enough for the common case.)

## Read-only safety boundary

`EngineConfig::read_only` defaults to `true`. The `Engine` refuses any command
whose `meta().mutating` is true while it's set. The TUI may expose a guarded
toggle; the MCP server leaves it on permanently so an agent can never install,
uninstall, reboot, or set properties.

## Implementation plan (phased)

**Phase 1 — foundation (this scaffold).** Workspace, parser trait + 4 parsers
with unit tests, command trait + transport + mock, registry/engine/poller,
minimal TUI overview view, MCP server with 5 tools. *Status: scaffolded.*

**Phase 2 — parser breadth.** Add the `dumpsys package` parser (permissions +
components, with the `exported` flag), `top`/`/proc` for processes, `dumpsys
meminfo`, `dumpsys connectivity`/`ip addr`. Each ships with fixture tests.

**Phase 3 — TUI depth.** Package list view with filter + lazy detail fetch on
select; process table sorted by CPU; live logcat view backed by a streaming job
(a `logcat` task writing into a bounded ring buffer rather than a polled
snapshot).

**Phase 4 — command breadth (ATK parity).** Port ATK actions as `Command`s,
classifying each as read-only or mutating. Group destructive ones (install,
uninstall, reboot, prop set) behind the read-only gate.

**Phase 5 — MCP polish.** Richer tool descriptions and structured outputs;
consider an opt-in mutating profile behind an explicit flag; add a "diagnose app"
prompt/tool that chains logcat + package detail for crash/ANR triage.

**Phase 6 — robustness.** Multi-device selection UX, reconnect handling,
graceful behavior when the adb server isn't running, configurable poll intervals,
and a config file.

## Testing strategy

- **Parsers:** fixture-string unit tests (already present).
- **Commands:** `MockTransport` end-to-end tests (already present for two).
- **Engine/registry:** unit tests for read-only enforcement and unknown-id
  handling.
- **Integration (optional, gated):** a feature flag that runs against a real
  emulator in CI.

## Why these dependencies

- `adb_client` — pure-Rust ADB protocol client, no shelling out to the `adb`
  binary; supports both adb-server and direct-USB modes.
- `ratatui` + `crossterm` — the de-facto Rust TUI stack; crossterm's
  `event-stream` feature gives async key events that compose with tokio.
- `rmcp` — the official Rust MCP SDK; `#[tool_router]`/`#[tool]` macros generate
  the protocol plumbing.
- `tokio` — async runtime underpinning the poller, transport, and both binaries.
