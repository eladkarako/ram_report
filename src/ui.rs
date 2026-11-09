use crate::{
    app::{AppState, MAX_HISTORY},
    memory,
};

use humansize::{format_size, BINARY};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{
        Block, Borders, Gauge, Paragraph, RenderDirection, Sparkline,
    },
    Frame,
};

const INFO_SECTION_HEIGHT: u16 = 3;
const GAUGE_SECTION_HEIGHT: u16 = 5;
const CHART_SECTION_HEIGHT: u16 = 5;

pub fn draw(
    frame: &mut Frame,
    state: &AppState,
) {
    let [gauge_area, info_area, chart_area] =
        section_areas(frame.area());
    frame.render_widget(memory_gauge(state), gauge_area);
    frame.render_widget(delta_paragraph(state), info_area);
    frame.render_widget(history_sparkline(state), chart_area);
}

fn section_areas(area: Rect) -> [Rect; 3] {
    let margin_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(GAUGE_SECTION_HEIGHT),
            Constraint::Length(INFO_SECTION_HEIGHT),
            Constraint::Length(CHART_SECTION_HEIGHT),
        ])
        .split(margin_chunks[1]);

    [chunks[0], chunks[1], chunks[2]]
}

fn memory_gauge(state: &AppState) -> Gauge<'static> {
    let current = state.current;
    Gauge::default()
        .percent(current.used_percent() as u16)
        .label(format!(
            "Used: {} [{:.2}%]  |  Free: {} [{:.2}%]",
            format_size(current.used, BINARY),
            current.used_percent(),
            format_size(current.free, BINARY),
            current.free_percent(),
        ))
        .block(Block::default().borders(Borders::ALL).title(
            format!(
                "◧ Memory Usage (Total: {})",
                format_size(state.reference.total, BINARY)
            ),
        ))
}

fn delta_paragraph(state: &AppState) -> Paragraph<'static> {
    let delta = memory::used_delta(state.current, state.reference);
    let delta_percent =
        memory::used_delta_percent(delta, state.reference);
    Paragraph::new(format_delta(delta, delta_percent)).block(
        Block::default().borders(Borders::ALL).title(format!(
            "∆ Delta Change to {} (Press R to reset)",
            format_size(state.reference.used, BINARY)
        )),
    )
}

fn format_delta(
    delta: i64,
    delta_percent: f64,
) -> String {
    if delta >= 0 {
        format!(
            "+{} [+{:.2}%]",
            format_size(delta as u64, BINARY),
            delta_percent
        )
    } else {
        format!(
            "-{} [{:.2}%]",
            format_size((-delta) as u64, BINARY),
            delta_percent.abs()
        )
    }
}

fn history_sparkline(state: &AppState) -> Sparkline<'static> {
    let data: Vec<u64> = state.history.iter().copied().collect();
    Sparkline::default()
        .direction(RenderDirection::RightToLeft)
        .data(data)
        .max(state.current.total)
        .block(
            Block::default().borders(Borders::ALL).title(format!(
                "◪ Memory History (latest {} updates, newest on left, oldest on right)",
                MAX_HISTORY
            )),
        )
}
