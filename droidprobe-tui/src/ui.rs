//! Rendering. Pure functions from `&App` to a ratatui frame — no state mutation
//! here, which keeps draw logic easy to reason about and the render loop cheap.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

use crate::app::{App, View};

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
        View::Packages => draw_placeholder(f, "Packages", chunks[1]),
        View::Logs => draw_placeholder(f, "Logs", chunks[1]),
    }

    let help = Paragraph::new(Line::from(vec![
        Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" switch view  "),
        Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" quit"),
    ]));
    f.render_widget(help, chunks[2]);
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

fn draw_overview(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let mut lines: Vec<Line> = Vec::new();

    // Device info, if polled yet.
    if let Some(info) = app.snapshot_ok("device.info") {
        let model = info.get("model").and_then(|v| v.as_str()).unwrap_or("?");
        let release = info
            .get("android_release")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let sdk = info.get("sdk").and_then(|v| v.as_u64()).unwrap_or(0);
        lines.push(Line::from(format!("Device:  {model}  (Android {release}, SDK {sdk})")));
    } else {
        lines.push(Line::from("Device:  fetching…"));
    }

    // Battery, if polled yet.
    if let Some(batt) = app.snapshot_ok("battery.status") {
        let level = batt.get("level").and_then(|v| v.as_u64()).unwrap_or(0);
        let src = batt
            .get("power_source")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let temp = batt
            .get("temperature_c")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        lines.push(Line::from(format!(
            "Battery: {level}%  source={src}  {temp:.1}°C"
        )));
    } else {
        lines.push(Line::from("Battery: fetching…"));
    }

    let body = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Overview"));
    f.render_widget(body, area);
}

fn draw_placeholder(f: &mut Frame, name: &str, area: ratatui::layout::Rect) {
    let body = Paragraph::new(format!("{name} view — not yet implemented"))
        .block(Block::default().borders(Borders::ALL).title(name));
    f.render_widget(body, area);
}
