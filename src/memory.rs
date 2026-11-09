use anyhow::{ensure, Context, Result};
use sysinfo::{MemoryRefreshKind, System};

const KILOBYTE_MULTIPLIER: u64 = 1024;

#[derive(Clone, Copy)]
pub struct RamSnapshot {
    pub free: u64,
    pub used: u64,
    pub total: u64,
}

impl RamSnapshot {
    pub fn used_percent(&self) -> f64 {
        (self.used as f64 / self.total as f64) * 100.0
    }

    pub fn free_percent(&self) -> f64 {
        (self.free as f64 / self.total as f64) * 100.0
    }
}

pub fn get_memory_snapshot(
    system: &mut System
) -> Result<RamSnapshot> {
    system.refresh_memory_specifics(
        MemoryRefreshKind::nothing().with_ram(),
    );

    let total = to_bytes(system.total_memory())
        .context("Total memory calculation overflow")?;
    let free = to_bytes(system.free_memory())
        .context("Free memory calculation overflow")?;
    ensure!(free <= total, "Free memory exceeds total memory");

    let used = total - free;
    Ok(RamSnapshot { free, used, total })
}

fn to_bytes(kilobytes: u64) -> Option<u64> {
    (kilobytes / 1_000).checked_mul(KILOBYTE_MULTIPLIER)
}

pub fn used_delta(
    current: RamSnapshot,
    reference: RamSnapshot,
) -> i64 {
    current.used as i64 - reference.used as i64
}

pub fn used_delta_percent(
    delta: i64,
    reference: RamSnapshot,
) -> f64 {
    if reference.used > 0 {
        (delta as f64 / reference.used as f64) * 100.0
    } else {
        0.0
    }
}
