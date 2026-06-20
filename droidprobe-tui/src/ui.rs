//! Rendering. Pure functions from `&App` to a ratatui frame — no state mutation
//! here, which keeps draw logic easy to reason about and the render loop cheap.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState, Tabs,
    },
    Frame,
};

use droidprobe_parser::model::{PackageDetail, ProtectionLevel};

use crate::app::{App, DetailTab, View};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // body
            Constraint::Length(1), // status/help line
        ])
        .split(f.area());

    draw_tabs(f, app, chunks[0]);

    match app.view {
        View::Overview => draw_overview(f, app, chunks[1]),
        View::Packages => draw_packages(f, app, chunks[1]),
        View::Logs => draw_placeholder(f, "Logs", chunks[1]),
    }

    let help = match app.view {
        View::Packages if app.packages.search_active => Line::from(vec![
            Span::styled("type", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" to filter  "),
            Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" keep filter  "),
            Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" cancel search"),
        ]),
        View::Packages => Line::from(vec![
            Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" switch view  "),
            Span::styled("↑↓", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" select  "),
            Span::styled("←→", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" tab  "),
            Span::styled("PgUp/PgDn", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" scroll  "),
            Span::styled("/", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" search  "),
            Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" quit"),
        ]),
        _ => Line::from(vec![
            Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" switch view  "),
            Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" quit"),
        ]),
    };
    f.render_widget(Paragraph::new(help), chunks[2]);
}

fn draw_tabs(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let titles: Vec<Line> = [View::Overview, View::Packages, View::Logs]
        .iter()
        .map(|v| Line::from(v.title()))
        .collect();
    let selected = match app.view {
        View::Overview => 0,
        View::Packages => 1,
        View::Logs => 2,
    };
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("droidprobe"))
        .select(selected)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_widget(tabs, area);
}

fn draw_overview(f: &mut Frame, app: &App, area: Rect) {
    const FETCHING: &str = "⏳  fetching…";

    let device = app.snapshot_ok("device.info");
    let cpu = app.snapshot_ok("device.cpu");
    let screen = app.snapshot_ok("device.screen");
    let memory = app.snapshot_ok("device.memory");
    let storage = app.snapshot_ok("device.storage");
    let imei = app.snapshot_ok("device.imei");
    let battery = app.snapshot_ok("battery.status");

    let str_field = |v: Option<&serde_json::Value>, key: &str| -> Option<String> {
        v.and_then(|v| v.get(key))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    let u64_field = |v: Option<&serde_json::Value>, key: &str| -> Option<u64> {
        v.and_then(|v| v.get(key)).and_then(|v| v.as_u64())
    };
    // "Not polled yet" and "polled but failed" both look like `None` through
    // `snapshot_ok`; check the error separately so a real failure (bad
    // command, parse error) doesn't just spin as "fetching…" forever.
    let pending = |id: &str| match app.snapshot_err(id) {
        Some(err) => format!("⚠️  error: {}", truncate(err, 50)),
        None => FETCHING.to_string(),
    };

    let model_row = match device {
        Some(_) => format!(
            "{}  ({} / {})",
            str_field(device, "model").unwrap_or_default(),
            str_field(device, "manufacturer").unwrap_or_default(),
            str_field(device, "brand").unwrap_or_default(),
        ),
        None => pending("device.info"),
    };

    let android_row = match device {
        Some(_) => format!(
            "{} (API {})",
            str_field(device, "android_release").unwrap_or_default(),
            u64_field(device, "sdk").unwrap_or(0),
        ),
        None => pending("device.info"),
    };

    let fingerprint_row = match str_field(device, "fingerprint") {
        Some(fp) if !fp.is_empty() => truncate(&fp, 56),
        _ if device.is_some() => "n/a".to_string(),
        _ => pending("device.info"),
    };

    let serial_row = match str_field(device, "serial") {
        Some(s) if !s.is_empty() => s,
        _ if device.is_some() => "n/a".to_string(),
        _ => pending("device.info"),
    };

    let imei_row = match str_field(imei, "imei") {
        Some(s) if !s.is_empty() => s,
        _ if imei.is_some() => "n/a (needs privileged permission)".to_string(),
        _ => pending("device.imei"),
    };

    let cpu_row = match cpu {
        Some(_) => {
            let abi = str_field(device, "abi").filter(|s| !s.is_empty());
            let hardware = str_field(cpu, "hardware").filter(|s| !s.is_empty());
            format!(
                "{}  ·  {} cores  ·  {}",
                abi.unwrap_or_else(|| "?".to_string()),
                u64_field(cpu, "core_count").unwrap_or(0),
                hardware.unwrap_or_else(|| "?".to_string()),
            )
        }
        None => pending("device.cpu"),
    };

    let screen_row = match screen {
        Some(_) => format!(
            "{}x{} @ {} dpi",
            u64_field(screen, "width").unwrap_or(0),
            u64_field(screen, "height").unwrap_or(0),
            u64_field(screen, "density_dpi").unwrap_or(0),
        ),
        None => pending("device.screen"),
    };

    let ram_row = match memory {
        Some(_) => format!(
            "{} free / {} total",
            human_kb(u64_field(memory, "available_kb").unwrap_or(0)),
            human_kb(u64_field(memory, "total_kb").unwrap_or(0)),
        ),
        None => pending("device.memory"),
    };

    let storage_row = match storage.and_then(|v| v.as_array()) {
        Some(entries) if !entries.is_empty() => {
            let data = entries
                .iter()
                .find(|e| e.get("mounted_on").and_then(|v| v.as_str()) == Some("/data"))
                .or_else(|| entries.first());
            match data {
                Some(e) => {
                    let size = e.get("size_kb").and_then(|v| v.as_u64()).unwrap_or(0);
                    let avail = e.get("available_kb").and_then(|v| v.as_u64()).unwrap_or(0);
                    let pct = e.get("use_percent").and_then(|v| v.as_u64()).unwrap_or(0);
                    format!(
                        "{} free / {} total ({pct}% used)",
                        human_kb(avail),
                        human_kb(size)
                    )
                }
                None => "n/a".to_string(),
            }
        }
        Some(_) => "n/a".to_string(),
        None => pending("device.storage"),
    };

    let battery_row = match battery {
        Some(_) => format!(
            "{}%  ·  {}  ·  {:.1}°C",
            u64_field(battery, "level").unwrap_or(0),
            str_field(battery, "power_source").unwrap_or_default(),
            battery
                .and_then(|v| v.get("temperature_c"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
        ),
        None => pending("battery.status"),
    };

    let rows = vec![
        Row::new(vec![Cell::from("🏷  Model"), Cell::from(model_row)]),
        Row::new(vec![Cell::from("🤖  Android"), Cell::from(android_row)]),
        Row::new(vec![
            Cell::from("🔏  Fingerprint"),
            Cell::from(fingerprint_row),
        ]),
        Row::new(vec![Cell::from("🔢  Serial"), Cell::from(serial_row)]),
        Row::new(vec![Cell::from("📞  IMEI"), Cell::from(imei_row)]),
        Row::new(vec![Cell::from("🧮  CPU"), Cell::from(cpu_row)]),
        Row::new(vec![Cell::from("🖥  Screen"), Cell::from(screen_row)]),
        Row::new(vec![Cell::from("💾  RAM"), Cell::from(ram_row)]),
        Row::new(vec![Cell::from("🗄  Storage"), Cell::from(storage_row)]),
        Row::new(vec![Cell::from("🔋  Battery"), Cell::from(battery_row)]),
    ];

    let table = Table::new(rows, [Constraint::Length(17), Constraint::Min(40)]).block(
        Block::default()
            .borders(Borders::ALL)
            .title("📱  Device Identity & Hardware"),
    );
    f.render_widget(table, area);
}

/// Render kilobytes as a human-friendly GB/MB string.
fn human_kb(kb: u64) -> String {
    let gb = kb as f64 / (1024.0 * 1024.0);
    if gb >= 1.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{:.0} MB", kb as f64 / 1024.0)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

fn draw_packages(f: &mut Frame, app: &App, area: Rect) {
    let total = app.package_list().len();
    let filtered = app.filtered_packages();

    if total == 0 {
        draw_package_list(f, app, &filtered, total, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    draw_package_list(f, app, &filtered, total, chunks[0]);
    if filtered.is_empty() {
        let body = Paragraph::new("no matches")
            .block(Block::default().borders(Borders::ALL).title("Details"));
        f.render_widget(body, chunks[1]);
    } else {
        draw_package_detail(f, app, chunks[1]);
    }
}

fn draw_package_list(
    f: &mut Frame,
    app: &App,
    packages: &[droidprobe_parser::model::PackageRef],
    total: usize,
    area: Rect,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let query = &app.packages.search;
    let search_line = if query.is_empty() {
        Line::from(Span::styled(
            "🔍  press / to search",
            Style::default().fg(Color::DarkGray),
        ))
    } else {
        let cursor = if app.packages.search_active {
            "▏"
        } else {
            ""
        };
        Line::from(format!("🔍  {query}{cursor}"))
    };
    let search_box = Paragraph::new(search_line)
        .style(if app.packages.search_active {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        })
        .block(Block::default().borders(Borders::ALL).title("Search"));
    f.render_widget(search_box, chunks[0]);

    let items: Vec<ListItem> = packages
        .iter()
        .map(|p| {
            let marker = if p.system { "[s] " } else { "" };
            ListItem::new(format!("{marker}{}", p.name))
        })
        .collect();

    let mut state = ListState::default();
    if !packages.is_empty() {
        state.select(Some(app.packages.selected.min(packages.len() - 1)));
    }

    let title = if query.is_empty() {
        format!("Packages ({total})")
    } else {
        format!("Packages ({}/{total})", packages.len())
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(list, chunks[1], &mut state);
}

fn draw_package_detail(f: &mut Frame, app: &App, area: Rect) {
    let title = app
        .packages
        .pending
        .as_deref()
        .or_else(|| app.packages.detail.as_ref().map(|d| d.name.as_str()))
        .unwrap_or("package");

    if app.packages.pending.is_some() {
        let body = Paragraph::new("⏳  fetching…")
            .block(Block::default().borders(Borders::ALL).title(title));
        f.render_widget(body, area);
        return;
    }

    if let Some(err) = &app.packages.detail_error {
        let body = Paragraph::new(Line::from(Span::styled(
            format!("❌  error: {err}"),
            Style::default().fg(Color::Red),
        )))
        .block(Block::default().borders(Borders::ALL).title(title));
        f.render_widget(body, area);
        return;
    }

    let Some(detail) = &app.packages.detail else {
        let body = Paragraph::new("select a package")
            .block(Block::default().borders(Borders::ALL).title(title));
        f.render_widget(body, area);
        return;
    };

    let dangerous = detail
        .permissions
        .iter()
        .filter(|p| p.protection_level == ProtectionLevel::Dangerous)
        .count();
    let granted = detail.permissions.iter().filter(|p| p.granted).count();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // status line
            Constraint::Length(1), // sub-tab bar
            Constraint::Min(0),    // active tab content
        ])
        .split(area);

    let status = Paragraph::new(Line::from(vec![
        Span::raw("📦  "),
        Span::styled(
            format!("v{}", detail.version_name),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" (code {})   ", detail.version_code)),
        Span::raw(format!(
            "🛠  SDK {}–{}   ",
            detail.min_sdk, detail.target_sdk
        )),
        Span::raw(format!(
            "🔑  {} perms  🔴  {dangerous} dangerous  ✅  {granted} granted",
            detail.permissions.len()
        )),
    ]))
    .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(status, chunks[0]);

    draw_detail_tab_bar(f, app, chunks[1]);

    match app.packages.detail_tab {
        DetailTab::Permissions => draw_permissions_table(f, app, detail, chunks[2]),
        tab => draw_component_table(f, app, tab, detail, chunks[2]),
    }
}

const DETAIL_TABS: [DetailTab; 5] = [
    DetailTab::Permissions,
    DetailTab::Activities,
    DetailTab::Services,
    DetailTab::Receivers,
    DetailTab::Providers,
];

fn draw_detail_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let spans: Vec<Span> = DETAIL_TABS
        .iter()
        .map(|tab| {
            let style = if *tab == app.packages.detail_tab {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Span::styled(format!(" {} ", tab.title()), style)
        })
        .collect();
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_permissions_table(f: &mut Frame, app: &App, detail: &PackageDetail, area: Rect) {
    let rows = detail.permissions.iter().map(|perm| {
        let (icon, color) = match perm.protection_level {
            ProtectionLevel::Dangerous => ("🔴", Color::Yellow),
            ProtectionLevel::Signature => ("🔏", Color::Magenta),
            ProtectionLevel::Normal => ("🟢", Color::Reset),
            ProtectionLevel::Unknown => ("❓", Color::DarkGray),
        };
        let level = format!("{:?}", perm.protection_level).to_lowercase();
        let granted = if perm.granted {
            "✅  granted"
        } else {
            "⛔  denied"
        };
        Row::new(vec![
            Cell::from(icon),
            Cell::from(perm.name.clone()),
            Cell::from(level),
            Cell::from(granted),
        ])
        .style(Style::default().fg(color))
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Min(30),
            Constraint::Length(11),
            Constraint::Length(11),
        ],
    )
    .header(
        Row::new(vec!["", "Permission", "Level", "Granted"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Permissions ({})", detail.permissions.len())),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = TableState::default();
    if !detail.permissions.is_empty() {
        state.select(Some(
            app.packages.perm_selected.min(detail.permissions.len() - 1),
        ));
    }
    f.render_stateful_widget(table, area, &mut state);
}

/// Activities/Services/Receivers/Providers tab content. Components only
/// land here if they declare an `<intent-filter>` — see
/// [`droidprobe_parser::model::Component`] for why (and why there's no
/// `exported` column).
fn draw_component_table(
    f: &mut Frame,
    app: &App,
    tab: DetailTab,
    detail: &PackageDetail,
    area: Rect,
) {
    let components = tab.components(detail).unwrap_or(&[]);

    if components.is_empty() {
        let msg = if tab == DetailTab::Providers {
            "no providers shown — providers have no intent-filter (invoked by\nauthority URI instead), so they never appear in resolver tables"
        } else {
            "none declared with an intent-filter"
        };
        let body = Paragraph::new(msg).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("{} (0)", tab.title())),
        );
        f.render_widget(body, area);
        return;
    }

    let launcher = detail.launcher_activity.as_deref();
    let rows = components.iter().map(|c| {
        let marker = if Some(c.name.as_str()) == launcher {
            "🚀 "
        } else {
            ""
        };
        let actions = if c.intent_actions.is_empty() {
            "—".to_string()
        } else {
            c.intent_actions.join(", ")
        };
        Row::new(vec![
            Cell::from(format!("{marker}{}", c.name)),
            Cell::from(actions),
            Cell::from(c.permission.clone().unwrap_or_else(|| "—".to_string())),
        ])
    });

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(35),
            Constraint::Percentage(25),
        ],
    )
    .header(
        Row::new(vec!["Name", "Intent Actions", "Permission"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().borders(Borders::ALL).title(format!(
        "{} ({})",
        tab.title(),
        components.len()
    )))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = TableState::default();
    state.select(Some(
        app.packages.component_selected.min(components.len() - 1),
    ));
    f.render_stateful_widget(table, area, &mut state);
}

fn draw_placeholder(f: &mut Frame, name: &str, area: ratatui::layout::Rect) {
    let body = Paragraph::new(format!("{name} view — not yet implemented"))
        .block(Block::default().borders(Borders::ALL).title(name));
    f.render_widget(body, area);
}
