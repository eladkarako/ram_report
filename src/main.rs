#[cfg(all(
    any(target_os = "linux", target_os = "windows"),
    any(
        target_arch = "x86_64",
        target_arch = "x86",
        target_arch = "aarch64"
    )
))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod app;

mod memory;

mod theme;

mod ui;

use anyhow::{ensure, Context, Result};

use app::{spawn_input_listener, spawn_sampler, AppState};
use crossterm::{
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use parking_lot::RwLock;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io::Stdout,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use sysinfo::System;
use theme::Theme;

const RENDER_INTERVAL_MS: u64 = 100;
const MAIN_LOOP_SLEEP_MS: u64 = 300;

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

    let mut system =
        System::new().context("Failed to initialise system info")?;
    let initial_snapshot = memory::get_memory_snapshot(&mut system)
        .context("Failed to get initial memory snapshot")?;
    ensure!(initial_snapshot.total != 0, "Total memory is zero");

    let app_state =
        Arc::new(RwLock::new(AppState::new(initial_snapshot)));
    let exit_flag = Arc::new(AtomicBool::new(false));
    let reset_flag = Arc::new(AtomicBool::new(false));

    // Detected once at startup; the environment does not change while running.
    let theme = Theme::detect();

    spawn_input_listener(
        Arc::clone(&exit_flag),
        Arc::clone(&reset_flag),
    );
    spawn_sampler(Arc::clone(&app_state), Arc::clone(&exit_flag));

    let mut last_render = Instant::now();
    while !exit_flag.load(Ordering::Relaxed) {
        if reset_flag.swap(false, Ordering::Relaxed) {
            app_state.write().reset_reference();
        }

        if last_render.elapsed()
            >= Duration::from_millis(RENDER_INTERVAL_MS)
        {
            terminal
                .draw(|f| ui::draw(f, &app_state.read(), &theme))
                .context("Failed to draw UI")?;
            last_render = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(
            MAIN_LOOP_SLEEP_MS,
        ));
    }

    Ok(())
}
