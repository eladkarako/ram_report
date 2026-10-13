#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "x86_64",
        target_arch = "x86",
        target_arch = "aarch64"
    )
))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use anyhow::{ensure, Context, Result};
use crossterm::{
    event::{Event, KeyCode},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use humansize::{format_size, BINARY};
use parking_lot::RwLock;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Gauge, Paragraph, Sparkline},
    Terminal,
};
use std::{
    collections::VecDeque,
    io::Stdout,
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    time::Duration,
};

// Update cycle timings (milliseconds)
const MAX_HISTORY: usize = 120;
const UPDATE_INTERVAL_MS: u64 = 500;
const RENDER_INTERVAL_MS: u64 = 100;
const INPUT_TIMEOUT_MS: u64 = 50;
const MAIN_LOOP_SLEEP_MS: u64 = 300;

// Layout constraints (height in lines)
const INFO_SECTION_HEIGHT: u16 = 3;
const GAUGE_SECTION_HEIGHT: u16 = 5;
const CHART_SECTION_HEIGHT: u16 = 5;

// Memory calculation factors
const KILOBYTE_MULTIPLIER: u64 = 1024;

#[derive(Clone, Copy)]
struct RamSnapshot {
    free: u64,
    used: u64,
    total: u64,
}

impl RamSnapshot {}

struct AppState {
    current: RamSnapshot,
    reference: RamSnapshot,
    history: VecDeque<u64>,
}

fn get_memory_snapshot() -> Result<RamSnapshot> {
    let mem = sys_info::mem_info()
        .context("Failed to read memory info")?;

    let total = mem
        .total
        .checked_mul(KILOBYTE_MULTIPLIER)
        .context("Total memory calculation overflow")?;
    let free = mem
        .free
        .checked_mul(KILOBYTE_MULTIPLIER)
        .context("Free memory calculation overflow")?;

    ensure!(free <= total, "Free memory exceeds total memory");
    let used = total - free;

    Ok(RamSnapshot { free, used, total })
}

fn main() -> Result<()> {
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)
        .context("Failed to enter alt screen")?;

    let result = run_app(&mut stdout);

    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(stdout, LeaveAlternateScreen)
        .context("Failed to leave alt screen")?;

    result
}

fn run_app(stdout: &mut Stdout) -> Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))
        .context("Failed to create terminal")?;

    let initial_snapshot = get_memory_snapshot()?;
    ensure!(initial_snapshot.total != 0, "Total memory is zero");

    let mut initial_history = VecDeque::with_capacity(MAX_HISTORY);
    initial_history.push_back(initial_snapshot.used);

    let app_state = Arc::new(RwLock::new(AppState {
        current: initial_snapshot,
        reference: initial_snapshot,
        history: initial_history,
    }));

    let exit_flag = Arc::new(AtomicBool::new(false));
    let reset_flag = Arc::new(AtomicBool::new(false));

    // Input thread
    let exit_flag_input = Arc::clone(&exit_flag);
    let reset_flag_input = Arc::clone(&reset_flag);
    std::thread::spawn(move || {
        while !exit_flag_input.load(Ordering::Relaxed) {
            // Poll with timeout instead of sleep
            if let Ok(true) = crossterm::event::poll(
                Duration::from_millis(INPUT_TIMEOUT_MS),
            ) {
                if let Ok(Event::Key(key)) = crossterm::event::read()
                {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            exit_flag_input
                                .store(true, Ordering::Relaxed);
                            break;
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            reset_flag_input
                                .store(true, Ordering::Relaxed);
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    // Data update thread with interval polling
    let app_state_data = Arc::clone(&app_state);
    let exit_flag_data = Arc::clone(&exit_flag);
    std::thread::spawn(move || {
        let mut last_update = std::time::Instant::now();
        while !exit_flag_data.load(Ordering::Relaxed) {
            // Poll-based timing instead of sleep
            if last_update.elapsed()
                >= Duration::from_millis(UPDATE_INTERVAL_MS)
            {
                match get_memory_snapshot() {
                    Ok(snapshot) => {
                        let mut state = app_state_data.write();
                        state.current = snapshot;
                        state.history.push_back(snapshot.used);

                        if state.history.len() > MAX_HISTORY {
                            state.history.pop_front();
                        }
                    }
                    Err(e) => {
                        eprintln!("Memory snapshot error: {}", e);
                    }
                }
                last_update = std::time::Instant::now();
            }
            // Small sleep to prevent busy-waiting
            std::thread::sleep(Duration::from_millis(10));
        }
    });

    // Main render loop with poll-based timing
    let mut last_render = std::time::Instant::now();
    while !exit_flag.load(Ordering::Relaxed) {
        if reset_flag.swap(false, Ordering::Relaxed) {
            let mut state = app_state.write();
            state.reference = state.current;
        }

        // Poll-based rendering instead of checking elapsed time
        if last_render.elapsed()
            >= Duration::from_millis(RENDER_INTERVAL_MS)
        {
            terminal
                .draw(|f| {
                    let state = app_state.read();

                    // Calculate delta from reference
                    let delta = state.current.used as i64
                        - state.reference.used as i64;
                    let delta_percent = if state.reference.used > 0 {
                        (delta as f64 / state.reference.used as f64) * 100.0
                    } else {
                        0.0
                    };
                    let delta_str = if delta >= 0 {
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
                    };

                    let free_percent = (state.current.free as f64 / state.current.total as f64) * 100.0;
                    let used_percent = (state.current.used as f64 / state.current.total as f64) * 100.0;

                    let margin_chunks = Layout::default()
                        .direction(Direction::Horizontal) //adds a right and left padding, it helps to avoid cropping in some terminals.
                        .constraints([
                            Constraint::Length(1),  // left margin
                            Constraint::Min(0),     // main content area
                            Constraint::Length(1),  // right margin
                        ])
                        .split(f.area());

                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(GAUGE_SECTION_HEIGHT),
                            Constraint::Length(INFO_SECTION_HEIGHT),
                            Constraint::Length(CHART_SECTION_HEIGHT),
                        ])
                        .split(margin_chunks[1]);

                    f.render_widget(
                        Gauge::default()
                            .percent(used_percent as u16)
                            .label(
                                format!("Used: {} [{:.2}%]  |  Free: {:} [{:.2}%]",
                                        format_size(state.current.used, BINARY),
                                        used_percent,
                                        format_size(state.current.free, BINARY),
                                        free_percent,
                                )
                            )
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .title("Memory Usage")
                            ),
                        chunks[0],
                    );

                    f.render_widget(
                        Paragraph::new(format!("{}", delta_str))
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .title(format!("∆ Delta Change to {} (Press R to reset)", format_size(state.reference.used, BINARY)))
                            ),
                        chunks[1],
                    );

                    f.render_widget(
                        Sparkline::default()
                            .data(
                                state
                                    .history
                                    .iter()
                                    .copied()
                                    .collect::<Vec<_>>(),
                            )
                            .max(state.current.total)
                            .block(
                                Block::default()
                                    .borders(Borders::ALL)
                                    .title("Memory History")
                            ),
                        chunks[2],
                    );
                })
                .context("Failed to draw UI")?;

            last_render = std::time::Instant::now();
        }

        std::thread::sleep(Duration::from_millis(
            MAIN_LOOP_SLEEP_MS,
        ));
    }

    Ok(())
}
