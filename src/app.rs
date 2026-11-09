use crate::memory::{self, RamSnapshot};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use parking_lot::RwLock;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use sysinfo::System;

pub const MAX_HISTORY: usize = 120;
const UPDATE_INTERVAL_MS: u64 = 500;
const INPUT_TIMEOUT_MS: u64 = 50;
const SAMPLER_SLEEP_MS: u64 = 10;

pub struct AppState {
    pub current: RamSnapshot,
    pub reference: RamSnapshot,
    pub history: VecDeque<u64>,
}

impl AppState {
    pub fn new(initial: RamSnapshot) -> Self {
        let mut history = VecDeque::with_capacity(MAX_HISTORY);
        history.push_back(initial.used);
        Self {
            current: initial,
            reference: initial,
            history,
        }
    }

    pub fn record_snapshot(
        &mut self,
        snapshot: RamSnapshot,
    ) {
        self.current = snapshot;
        self.history.push_front(snapshot.used);
        if self.history.len() > MAX_HISTORY {
            self.history.pop_back();
        }
    }

    pub fn reset_reference(&mut self) {
        self.reference = self.current;
    }
}

pub fn spawn_sampler(
    app_state: Arc<RwLock<AppState>>,
    exit_flag: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut system = match System::new() {
            Ok(system) => system,
            Err(e) => {
                eprintln!("Failed to initialise sysinfo: {}", e);
                return;
            }
        };
        let mut last_update = Instant::now();

        while !exit_flag.load(Ordering::Relaxed) {
            if last_update.elapsed()
                >= Duration::from_millis(UPDATE_INTERVAL_MS)
            {
                sample_into(&mut system, &app_state);
                last_update = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(
                SAMPLER_SLEEP_MS,
            ));
        }
    });
}

fn sample_into(
    system: &mut System,
    app_state: &RwLock<AppState>,
) {
    match memory::get_memory_snapshot(system) {
        Ok(snapshot) => app_state.write().record_snapshot(snapshot),
        Err(e) => eprintln!("Memory snapshot error: {}", e),
    }
}

pub fn spawn_input_listener(
    exit_flag: Arc<AtomicBool>,
    reset_flag: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        while !exit_flag.load(Ordering::Relaxed) {
            match next_key_action() {
                KeyAction::Exit => {
                    exit_flag.store(true, Ordering::Relaxed);
                    break;
                }
                KeyAction::Reset => {
                    reset_flag.store(true, Ordering::Relaxed)
                }
                KeyAction::Ignore => {}
            }
        }
    });
}

enum KeyAction {
    Exit,
    Reset,
    Ignore,
}

fn next_key_action() -> KeyAction {
    if let Ok(true) = crossterm::event::poll(Duration::from_millis(
        INPUT_TIMEOUT_MS,
    )) {
        if let Ok(Event::Key(key)) = crossterm::event::read() {
            return key_action(key);
        }
    }
    KeyAction::Ignore
}

fn key_action(key: KeyEvent) -> KeyAction {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => KeyAction::Exit,
        KeyCode::Char('r') | KeyCode::Char('R') => KeyAction::Reset,
        #[cfg(target_os = "windows")]
        KeyCode::F(4)
        if key.modifiers.contains(KeyModifiers::ALT) =>
            {
                KeyAction::Exit
            }
        KeyCode::Char('c')
        if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                KeyAction::Exit
            }
        _ => KeyAction::Ignore,
    }
}
