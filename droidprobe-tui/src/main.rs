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

use droidprobe_core::AdbTransport;
use droidprobe_core::{
    engine::{Engine, EngineConfig},
    poller::Poller,
    registry::Registry,
};
use droidprobe_parser::model::PackageDetail;

use app::App;
use app::DetailTab;
use app::View;

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
    poller.add_job(
        "package.list",
        None,
        serde_json::Value::Null,
        Duration::from_secs(30),
    );
    // Hardware identity (CPU, screen, IMEI) never changes during a session;
    // a long interval just guards against a stale read on a very long run.
    poller.add_job(
        "device.cpu",
        None,
        serde_json::Value::Null,
        Duration::from_secs(300),
    );
    poller.add_job(
        "device.screen",
        None,
        serde_json::Value::Null,
        Duration::from_secs(300),
    );
    poller.add_job(
        "device.imei",
        None,
        serde_json::Value::Null,
        Duration::from_secs(300),
    );
    // RAM/storage usage actually fluctuates, so poll those more often.
    poller.add_job(
        "device.memory",
        None,
        serde_json::Value::Null,
        Duration::from_secs(15),
    );
    poller.add_job(
        "device.storage",
        None,
        serde_json::Value::Null,
        Duration::from_secs(30),
    );
    let mut updates = poller.subscribe();

    // One-shot detail fetches (e.g. "show permissions for the package I just
    // selected") don't fit the recurring-interval shape of `Poller`, so they
    // get their own channel, merged into the same select loop below.
    let (detail_tx, mut detail_rx) =
        tokio::sync::mpsc::unbounded_channel::<(String, Result<PackageDetail, String>)>();

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
                        let searching = app.view == View::Packages && app.packages.search_active;
                        match key.code {
                            KeyCode::Esc if searching => {
                                app.packages.search_active = false;
                                app.packages.search.clear();
                                app.packages.selected = 0;
                            }
                            KeyCode::Enter if searching => {
                                app.packages.search_active = false;
                            }
                            KeyCode::Backspace if searching => {
                                app.packages.search.pop();
                                app.packages.selected = 0;
                            }
                            KeyCode::Char(c) if searching => {
                                app.packages.search.push(c);
                                app.packages.selected = 0;
                            }
                            KeyCode::Char('q') | KeyCode::Esc => app.running = false,
                            KeyCode::Tab => app.next_view(),
                            KeyCode::Char('/') if app.view == View::Packages => {
                                app.packages.search_active = true;
                            }
                            KeyCode::Up if app.view == View::Packages => {
                                let len = app.filtered_packages().len();
                                app.packages.move_selection(-1, len);
                            }
                            KeyCode::Down if app.view == View::Packages => {
                                let len = app.filtered_packages().len();
                                app.packages.move_selection(1, len);
                            }
                            KeyCode::PageUp if app.view == View::Packages => {
                                move_active_tab_selection(&mut app, -5);
                            }
                            KeyCode::PageDown if app.view == View::Packages => {
                                move_active_tab_selection(&mut app, 5);
                            }
                            KeyCode::Left if app.view == View::Packages => {
                                app.packages.cycle_tab(false);
                            }
                            KeyCode::Right if app.view == View::Packages => {
                                app.packages.cycle_tab(true);
                            }
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
            // One-shot package detail fetches.
            Some((name, result)) = detail_rx.recv() => {
                app.packages.apply_detail_result(&name, result);
            }
        }

        // Keep the detail pane in sync with whatever row is selected — no
        // separate "view details" action, it just follows the cursor like a
        // preview pane.
        sync_package_detail(&mut app, &engine, &detail_tx);
    }

    restore_terminal(&mut terminal)?;
    Ok(())
}

/// If the row under the cursor in the packages view isn't the one currently
/// fetched (or being fetched), kick off a fetch for it. Cheap to call every
/// loop iteration — it's a no-op once the detail pane already matches.
fn sync_package_detail(
    app: &mut App,
    engine: &Engine,
    tx: &tokio::sync::mpsc::UnboundedSender<(String, Result<PackageDetail, String>)>,
) {
    if app.view != View::Packages {
        return;
    }
    let Some(pkg) = app.filtered_packages().get(app.packages.selected).cloned() else {
        return;
    };
    let current = app
        .packages
        .pending
        .clone()
        .or_else(|| app.packages.detail.as_ref().map(|d| d.name.clone()));
    if current.as_deref() == Some(pkg.name.as_str()) {
        return;
    }

    app.packages.pending = Some(pkg.name.clone());
    app.packages.detail = None;
    app.packages.detail_error = None;

    let engine = engine.clone();
    let tx = tx.clone();
    let name = pkg.name;
    tokio::spawn(async move {
        let result = engine
            .run(
                "package.detail",
                None,
                serde_json::Value::String(name.clone()),
            )
            .await
            .map_err(|e| e.to_string())
            .and_then(|json| serde_json::from_value(json).map_err(|e| e.to_string()));
        let _ = tx.send((name, result));
    });
}

/// Moves the selection in whichever detail sub-pane is active: the
/// permissions table, or one of the component tabs (Activities etc).
fn move_active_tab_selection(app: &mut App, delta: isize) {
    let Some(detail) = app.packages.detail.as_ref() else {
        return;
    };
    match app.packages.detail_tab {
        DetailTab::Permissions => {
            let len = detail.permissions.len();
            app.packages.move_perm_selection(delta, len);
        }
        tab => {
            let len = tab.components(detail).map_or(0, <[_]>::len);
            app.packages.move_component_selection(delta, len);
        }
    }
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

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    use crossterm::{execute, terminal::*};
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
