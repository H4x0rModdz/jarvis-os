//! System-resource probe: free disk on `/`, free RAM, swap usage.
//!
//! Reads two zero-dep sources:
//!
//!   - `/proc/meminfo` for MemTotal / MemAvailable / SwapTotal /
//!     SwapFree. Same surface every Linux memory monitor consumes.
//!   - `libc::statvfs("/")` for root-filesystem free space. Doing
//!     it via libc instead of shelling out to `df` keeps the
//!     daemon stable when df isn't in PATH (bootc minimal hosts).
//!
//! Failures (file unreadable, statvfs errno) leave the matching
//! `Signals` field as `None` so the rules guard with `?` and
//! simply don't fire — no nudges on a transient probe failure.

use crate::proactive::{Probe, Signals};
use async_trait::async_trait;
use std::ffi::CString;

pub struct SystemProbe;

#[async_trait]
impl Probe for SystemProbe {
    async fn snapshot(&self) -> Signals {
        // Compute the three optionals up front so the final struct
        // literal is one expression — clippy's
        // field_reassign_with_default flags the partial-then-mutate
        // pattern.
        let (mem_free_pct, swap_used_pct) = match read_mem_pcts() {
            Some((free, _total, swap)) => (Some(free), swap),
            None => (None, None),
        };
        // Probe /var, NOT "/". On a bootc/ostree system "/" is a
        // read-only composefs image (the packed /usr) that sits at
        // ~100% full BY DESIGN — statvfs("/") there reports near-zero
        // free and fired a false "disk crítico". The real writable
        // space (user data, containers, logs) lives on /var, which is
        // the actual backing filesystem. Fall back to "/" only if
        // /var can't be stat'd (non-ostree host).
        let disk_root_free_pct = statvfs_free_pct("/var").or_else(|| statvfs_free_pct("/"));
        Signals {
            mem_free_pct,
            swap_used_pct,
            disk_root_free_pct,
            ..Default::default()
        }
    }
}

/// Parse `/proc/meminfo`. Returns `(free_pct, total_kb, swap_used_pct_opt)`.
/// `swap_used_pct` is None when the host has no swap.
fn read_mem_pcts() -> Option<(f64, u64, Option<f64>)> {
    let raw = std::fs::read_to_string("/proc/meminfo").ok()?;
    let mut mem_total: Option<u64> = None;
    let mut mem_available: Option<u64> = None;
    let mut swap_total: Option<u64> = None;
    let mut swap_free: Option<u64> = None;
    for line in raw.lines() {
        let mut parts = line.split_whitespace();
        let key = parts.next()?;
        let val = parts.next()?;
        let n: u64 = val.parse().ok()?;
        match key {
            "MemTotal:" => mem_total = Some(n),
            "MemAvailable:" => mem_available = Some(n),
            "SwapTotal:" => swap_total = Some(n),
            "SwapFree:" => swap_free = Some(n),
            _ => {}
        }
    }
    let mt = mem_total?;
    let ma = mem_available?;
    if mt == 0 {
        return None;
    }
    let free_pct = (ma as f64 / mt as f64) * 100.0;
    let swap_used = swap_total.and_then(|st| {
        if st == 0 {
            None
        } else {
            let used = st.saturating_sub(swap_free?);
            Some((used as f64 / st as f64) * 100.0)
        }
    });
    Some((free_pct, mt, swap_used))
}

/// `statvfs(path)` → free-blocks / total-blocks × 100. Returns
/// None when the syscall errors (path missing, permission, etc.).
fn statvfs_free_pct(path: &str) -> Option<f64> {
    let c_path = CString::new(path).ok()?;
    // statvfs has to be zero-initialised before the call — libc
    // populates it on success.
    let mut buf: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut buf) };
    if rc != 0 {
        return None;
    }
    let total = buf.f_blocks as f64;
    let free = buf.f_bavail as f64; // bavail = available to non-root
    if total == 0.0 {
        return None;
    }
    Some((free / total) * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn snapshot_returns_values_on_a_real_linux_host() {
        // We're running on a Linux box during CI / local dev, so
        // /proc/meminfo + statvfs("/") should both succeed. On
        // non-Linux hosts the probe just returns None — that's the
        // correct degradation, not a failure to assert on.
        let probe = SystemProbe;
        let signals = probe.snapshot().await;
        #[cfg(target_os = "linux")]
        {
            assert!(
                signals.mem_free_pct.is_some(),
                "mem_free_pct should populate on linux"
            );
            assert!(
                signals.disk_root_free_pct.is_some(),
                "disk_root_free_pct should populate on linux"
            );
            let m = signals.mem_free_pct.unwrap();
            assert!((0.0..=100.0).contains(&m), "mem pct out of range: {m}");
            let d = signals.disk_root_free_pct.unwrap();
            assert!((0.0..=100.0).contains(&d), "disk pct out of range: {d}");
        }
        // swap_used_pct intentionally not asserted — VMs / containers
        // routinely have no swap configured, which is a valid None.
        let _ = signals;
    }
}
