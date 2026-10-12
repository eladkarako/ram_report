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

use anyhow::Result;
use bytesize::ByteSize;
use crossterm::{
    event::{Event, EventStream, KeyCode},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use futures::StreamExt;
use ratatui::{
    backend::CrosstermBackend, widgets::Paragraph, Terminal,
};
use std::time::Duration;
use std::{io, sync::Arc};
use sysinfo::System;
use tokio::{
    sync::{broadcast, Mutex},
    time::sleep,
};

#[cfg(target_os = "windows")]
use std::mem;

#[cfg(target_os = "windows")]
use winapi::{
    ctypes::c_void,
    shared::minwindef::FALSE,
    um::{
        handleapi::CloseHandle,
        processthreadsapi::OpenProcess,
        psapi::EmptyWorkingSet,
        tlhelp32::{
            CreateToolhelp32Snapshot, Process32FirstW,
            Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
        },
        wincon::{SetConsoleCP, SetConsoleOutputCP},
        winnt::{PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA},
    },
};

const HISTOGRAM_WIDTH: usize = 22;

struct AppState {
    history: Vec<u64>,
    baseline: u64,
    start_ram: u64,
    current_ram: u64,
    total_ram: u64,
    is_trimming: bool,
    trim_status: Vec<(u32, String, bool)>,
    progress_count: u64,
    progress_total: u64,
}

fn format_bytes(b: u64) -> String {
    ByteSize(b).to_string()
}

fn format_delta(delta_bytes: u64) -> String {
    let delta_gb = delta_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if delta_gb >= 1.0 {
        format!("{:.2}GB", delta_gb)
    } else {
        let delta_mb = delta_bytes as f64 / (1024.0 * 1024.0);
        format!("{:.2}MB", delta_mb)
    }
}

fn get_ram_usage() -> (u64, u64) {
    let mut sys = System::new_all();
    sys.refresh_all();
    let total = sys.total_memory();
    let used = sys.used_memory();
    (used, total)
}

fn build_history_indicator(
    history: &[u64],
    total: u64,
) -> String {
    if total == 0 {
        return "Hist: [░░░░░░░░░░░░░░░░░░░░]".to_string();
    }
    let indicators: String = history
        .iter()
        .map(|&u| {
            let pct = (u * 100) / total;
            match pct {
                90..=100 => '█',
                80..=89 => '▇',
                70..=79 => '▆',
                60..=69 => '▅',
                50..=59 => '▄',
                40..=49 => '▃',
                30..=39 => '▂',
                _ => '░',
            }
        })
        .collect();
    format!("Hist: [{}]", indicators)
}

#[cfg(target_os = "windows")]
fn set_console_utf8() {
    unsafe {
        SetConsoleCP(65001);
        SetConsoleOutputCP(65001);
    }
}

#[cfg(not(target_os = "windows"))]
fn set_console_utf8() {}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    set_console_utf8();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    let state = Arc::new(Mutex::new(AppState {
        history: vec![0; HISTOGRAM_WIDTH],
        baseline: 0,
        start_ram: 0,
        current_ram: 0,
        total_ram: 0,
        is_trimming: false,
        trim_status: Vec::new(),
        progress_count: 0,
        progress_total: 0,
    }));

    let (tx, rx) = broadcast::channel(1);
    let (render_tx, render_rx) = tokio::sync::mpsc::channel(1);

    {
        let (used, total) = get_ram_usage();
        let mut s = state.lock().await;
        s.current_ram = used;
        s.total_ram = total;
        s.start_ram = used;
        s.baseline = used;
        s.history[0] = used;
    }

    spawn_ram_monitor(Arc::clone(&state));
    spawn_input_handler(Arc::clone(&state), tx, render_tx);
    run_main_loop(Arc::clone(&state), rx, render_rx, terminal)
        .await?;

    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen)?;
    Ok(())
}

fn spawn_ram_monitor(state: Arc<Mutex<AppState>>) {
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(2)).await;
            let (used, total) = get_ram_usage();
            let mut s = state.lock().await;
            s.current_ram = used;
            s.total_ram = total;
            s.history.remove(0);
            s.history.push(used);
        }
    });
}

#[allow(unused_variables)]
fn spawn_input_handler(
    state: Arc<Mutex<AppState>>,
    tx: broadcast::Sender<()>,
    render_tx: tokio::sync::mpsc::Sender<()>,
) {
    tokio::spawn(async move {
        let mut reader = EventStream::new();
        while let Some(Ok(evt)) = reader.next().await {
            match evt {
                Event::Key(k) => match k.code {
                    KeyCode::Char('q')
                    | KeyCode::Char('x')
                    | KeyCode::Esc => {
                        let _ = tx.send(());
                        return;
                    }
                    KeyCode::Char('r') => {
                        let mut s = state.lock().await;
                        s.baseline = s.current_ram;
                    }
                    #[cfg(target_os = "windows")]
                    KeyCode::Char('w') => {
                        let mut s = state.lock().await;
                        if !s.is_trimming {
                            s.is_trimming = true;
                            let state_trim = Arc::clone(&state);
                            tokio::spawn(async move {
                                trim_all(state_trim).await;
                            });
                            let _ = render_tx.send(());
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    });
}

async fn run_main_loop(
    state: Arc<Mutex<AppState>>,
    mut rx: broadcast::Receiver<()>,
    mut render_rx: tokio::sync::mpsc::Receiver<()>,
    mut terminal: Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<()> {
    loop {
        tokio::select! {
            _=rx.recv()=>{
                break;
            }
            _=render_rx.recv()=>{
                render_frame(&mut terminal, Arc::clone(&state)).await?;
            }
            _=sleep(Duration::from_secs(2))=>{
                render_frame(&mut terminal, Arc::clone(&state)).await?;
            }
        }
    }
    Ok(())
}

async fn render_frame(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: Arc<Mutex<AppState>>,
) -> Result<()> {
    let s = state.lock().await;

    if s.is_trimming {
        render_trim_status(terminal, &s)?;
    } else {
        render_ram_stats(terminal, &s)?;
    }
    drop(s);

    Ok(())
}

fn render_ram_stats(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &AppState,
) -> Result<()> {
    let (used, total) = get_ram_usage();
    let delta = if state.baseline > used {
        (state.baseline - used, "-")
    } else {
        (used - state.baseline, "+")
    };

    let pct =
        if total > 0 { ((used * 100) / total) as u64 } else { 0 };

    let ram_line = format!(
        "RAM: {} / {} | Δ: {}{}",
        format_bytes(used),
        format_bytes(total),
        delta.1,
        format_delta(delta.0)
    );

    let bar_line = create_ram_bar_display(pct);
    let history_line =
        build_history_indicator(&state.history, state.total_ram);

    let output =
        format!("{}\n{}\n{}", ram_line, bar_line, history_line);

    terminal.draw(|f| {
        let paragraph = Paragraph::new(output);
        f.render_widget(paragraph, f.area());
    })?;

    Ok(())
}

fn create_ram_bar_display(pct: u64) -> String {
    let filled = (pct / 5) as usize;
    let empty = 20 - filled;
    let bar = format!(
        "[{}{}] {}%",
        "█".repeat(filled),
        "░".repeat(empty),
        pct
    );
    bar
}

fn render_trim_status(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &AppState,
) -> Result<()> {
    let mut output = String::from("=== Memory Trim ===\n");

    for (pid, path, done) in &state.trim_status {
        let sym = if *done { "✓" } else { "◆" };
        output.push_str(&format!(
            "[{}] PID {} - {}\n",
            sym, pid, path
        ));
    }

    let progress_line = format!(
        "\n[{}] {}/{} processes trimmed",
        create_progress_bar(
            state.progress_count,
            state.progress_total
        ),
        state.progress_count,
        state.progress_total
    );
    output.push_str(&progress_line);

    terminal.draw(|f| {
        let paragraph = Paragraph::new(output);
        f.render_widget(paragraph, f.area());
    })?;

    Ok(())
}

fn create_progress_bar(
    current: u64,
    total: u64,
) -> String {
    if total == 0 {
        return "░░░░░░░░░░░░░░░░░░░░".to_string();
    }
    let filled = ((current * 40) / total) as usize;
    let empty = 40 - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

#[cfg(target_os = "windows")]
async fn trim_all(state: Arc<Mutex<AppState>>) {
    let procs = enumerate_processes();
    initialize_trim_status(Arc::clone(&state), &procs).await;
    spawn_trim_tasks(Arc::clone(&state), procs).await;
    finalize_trim(Arc::clone(&state)).await;
    sleep(Duration::from_secs(3)).await;

    let mut s = state.lock().await;
    s.is_trimming = false;
}

#[cfg(target_os = "windows")]
async fn initialize_trim_status(
    state: Arc<Mutex<AppState>>,
    procs: &[ProcInfo],
) {
    let mut s = state.lock().await;
    s.trim_status = procs
        .iter()
        .map(|p| (p.pid, p.path.clone(), false))
        .collect();
    s.progress_total = procs.len() as u64;
    s.progress_count = 0;
}

#[cfg(target_os = "windows")]
async fn spawn_trim_tasks(
    state: Arc<Mutex<AppState>>,
    procs: Vec<ProcInfo>,
) {
    let sem = Arc::new(tokio::sync::Semaphore::new(4));
    let mut handles = Vec::new();

    for proc in procs {
        let sem_c = Arc::clone(&sem);
        let state_c = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            let _permit = sem_c.acquire().await;
            execute_trim_task(proc.pid, state_c).await;
            sleep(Duration::from_millis(10)).await;
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }
}

#[cfg(target_os = "windows")]
async fn execute_trim_task(
    pid: u32,
    state: Arc<Mutex<AppState>>,
) {
    let ok = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::task::spawn_blocking(move || trim_proc(pid)),
    )
        .await
        .is_ok();
    update_trim_status(state, pid, ok).await;
}

#[cfg(target_os = "windows")]
async fn update_trim_status(
    state: Arc<Mutex<AppState>>,
    pid: u32,
    success: bool,
) {
    let mut s = state.lock().await;
    if let Some(entry) =
        s.trim_status.iter_mut().find(|(p, _, _)| *p == pid)
    {
        entry.2 = success;
    }
    s.progress_count += 1;
}

#[cfg(target_os = "windows")]
async fn finalize_trim(state: Arc<Mutex<AppState>>) {
    let mut s = state.lock().await;
    s.progress_count = s.progress_total;
}

#[cfg(target_os = "windows")]
fn trim_proc(pid: u32) -> bool {
    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_SET_QUOTA,
            FALSE as i32,
            pid,
        );
        if handle.is_null() {
            return false;
        }
        execute_trim_syscalls(handle);
        CloseHandle(handle);
        true
    }
}

#[cfg(target_os = "windows")]
unsafe fn execute_trim_syscalls(handle: *mut c_void) {
    let _ = unsafe { EmptyWorkingSet(handle) };
}

#[cfg(target_os = "windows")]
struct ProcInfo {
    pid: u32,
    path: String,
}

#[cfg(target_os = "windows")]
fn enumerate_processes() -> Vec<ProcInfo> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap.is_null() {
            return Vec::new();
        }
        let procs = collect_process_entries(snap);
        CloseHandle(snap);
        procs
    }
}

#[cfg(target_os = "windows")]
unsafe fn collect_process_entries(
    snap: *mut c_void
) -> Vec<ProcInfo> {
    let mut procs = Vec::new();
    let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
    entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;

    if unsafe { Process32FirstW(snap, &mut entry) } == FALSE as i32 {
        return procs;
    }

    loop {
        let name = unsafe { extract_process_name(&entry) };
        procs
            .push(ProcInfo { pid: entry.th32ProcessID, path: name });

        if unsafe { Process32NextW(snap, &mut entry) }
            == FALSE as i32
        {
            break;
        }
    }
    procs
}

#[cfg(target_os = "windows")]
unsafe fn extract_process_name(entry: &PROCESSENTRY32W) -> String {
    let wide_slice = &entry.szExeFile[..];
    String::from_utf16_lossy(
        wide_slice
            .iter()
            .position(|&c| c == 0)
            .map(|pos| &wide_slice[..pos])
            .unwrap_or(wide_slice),
    )
        .to_string()
}

#[cfg(not(target_os = "windows"))]
async fn trim_all(state: Arc<Mutex<AppState>>) {
    // No-op on non-Windows platforms
    let mut s = state.lock().await;
    s.is_trimming = false;
}
