//! Terminal UI front-end.
//!
//! Architecture (the standard ratatui + tokio shape):
//! - the **main task** owns the `App` state and the ratatui `Terminal`;
//! - a **poller** (from `droidprobe-core`) runs commands on intervals and pushes
//!   `PollUpdate`s onto a broadcast channel;
//! - **crossterm's async `EventStream`** yields key events;
//! - a `tokio::select!` loop merges the two sources: key events mutate state or
//!   quit, poll updates refresh cached data; after either, we redraw.
//!
//! This file is deliberately a thin, working skeleton: one device-overview view
//! wired end-to-end. Additional views (packages list/detail, processes, logcat)
//! slot in as more `View` variants plus their own poll jobs.

mod app;
mod ui;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};

use droidprobe_core::{
    engine::{Engine, EngineConfig},
    poller::Poller,
    registry::Registry,
};
use droidprobe_core::AdbTransport;

use app::App;

#[tokio::main]
async fn main() -> Result<()> {
    // Logs go to a file, never stdout — stdout belongs to the TUI.
    init_tracing();

    // Build the core stack: transport -> registry -> engine -> poller.
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

    let mut poller = Poller::new(engine.clone(), 256);
    // Poll device info slowly and battery a bit faster.
    poller.add_job(
        "device.info",
        None,
        serde_json::Value::Null,
        Duration::from_secs(30),
    );
    poller.add_job(
        "battery.status",
        None,
        serde_json::Value::Null,
        Duration::from_secs(5),
    );
    let mut updates = poller.subscribe();

    // Enter the alternate screen / raw mode.
    let mut terminal = setup_terminal()?;
    let mut app = App::new();
    let mut events = EventStream::new();

    // Main loop.
    while app.running {
        terminal.draw(|f| ui::draw(f, &app))?;

        tokio::select! {
            // Keyboard / terminal events.
            maybe_event = events.next() => {
                if let Some(Ok(Event::Key(key))) = maybe_event {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => app.running = false,
                            KeyCode::Tab => app.next_view(),
                            _ => {}
                        }
                    }
                }
            }
            // Poll results.
            update = updates.recv() => {
                if let Ok(update) = update {
                    app.apply_update(update);
                }
            }
        }
    }

    restore_terminal(&mut terminal)?;
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    // Write to a file in the working dir; the TUI owns the terminal.
    if let Ok(file) = std::fs::File::create("droidprobe.log") {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .with_writer(file)
            .with_ansi(false)
            .try_init();
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>> {
    use crossterm::{execute, terminal::*};
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<()> {
    use crossterm::{execute, terminal::*};
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
