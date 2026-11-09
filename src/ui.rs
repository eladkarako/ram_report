use crate::{
    app::{AppState, MAX_HISTORY},
    memory,
    theme::Theme,
};
use humansize::{format_size, BINARY};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
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
    theme: &Theme,
) {
    let [gauge_area, info_area, chart_area] =
        section_areas(frame.area());
    frame.render_widget(memory_gauge(state, theme), gauge_area);
    frame.render_widget(delta_paragraph(state, theme), info_area);
    frame.render_widget(history_sparkline(state, theme), chart_area);
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

fn titled_block(
    title: String,
    theme: &Theme,
) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme.border))
        .title(Line::from(Span::styled(
            title,
            Style::new()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        )))
}

fn memory_gauge(
    state: &AppState,
    theme: &Theme,
) -> Gauge<'static> {
    let current = state.current;
    let percent = current.used_percent();

    let label = format!(
        "Used: {} [{:.2}%]  |  Free: {} [{:.2}%]",
        format_size(current.used, BINARY),
        current.used_percent(),
        format_size(current.free, BINARY),
        current.free_percent(),
    );

    Gauge::default()
        .percent(percent as u16)
        .gauge_style(Style::new().fg(theme.usage_color(percent)))
        .style(Style::new().fg(theme.text))
        .label(Span::styled(
            label,
            Style::new().add_modifier(Modifier::BOLD),
        ))
        .block(titled_block(
            format!(
                "◧ Memory Usage (Total: {})",
                format_size(state.reference.total, BINARY)
            ),
            theme,
        ))
}

fn delta_paragraph(
    state: &AppState,
    theme: &Theme,
) -> Paragraph<'static> {
    let delta = memory::used_delta(state.current, state.reference);
    let delta_percent =
        memory::used_delta_percent(delta, state.reference);

    let color = if delta > 0 {
        theme.increase
    } else if delta < 0 {
        theme.decrease
    } else {
        theme.text
    };

    Paragraph::new(Line::from(Span::styled(
        format_delta(delta, delta_percent),
        Style::new().fg(color).add_modifier(Modifier::BOLD),
    )))
        .block(titled_block(
            format!(
                "∆ Delta Change to {} (Press R to reset)",
                format_size(state.reference.used, BINARY)
            ),
            theme,
        ))
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

fn history_sparkline(
    state: &AppState,
    theme: &Theme,
) -> Sparkline<'static> {
    let data: Vec<u64> = state.history.iter().copied().collect();

    Sparkline::default()
        .direction(RenderDirection::RightToLeft)
        .data(data)
        .max(state.current.total)
        .style(Style::new().fg(theme.usage_color(state.current.used_percent())))
        .block(titled_block(
            format!(
                "◪ Memory History (latest {} updates, newest on left, oldest on right)",
                MAX_HISTORY
            ),
            theme,
        ))
}
