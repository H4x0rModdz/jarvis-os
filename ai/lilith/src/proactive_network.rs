//! Connectivity probe via `/proc/net/route`.
//!
//! `has_connectivity = true` when at least one route in the kernel
//! table has Destination = 00000000 (the default route, 0.0.0.0)
//! and its Flags byte includes RTF_UP. That's a strong signal we
//! can reach anything outside the LAN — short of an actual DNS
//! lookup which would add latency every 30 s.
//!
//! Why not full DNS reachability? A successful `getaddrinfo` would
//! cost ~10-50 ms per tick and could itself hang on a misbehaving
//! resolver. The default-route check is what NetworkManager / iwd
//! use as the "online" hint, and it's enough for the edge rule
//! (network_lost) to do its job. If a user really has a default
//! route but no internet (captive portal), that's a different rule
//! entirely and lands as future work.

use crate::proactive::{Probe, Signals};
use async_trait::async_trait;

pub struct NetworkProbe;

const PROC_ROUTE: &str = "/proc/net/route";
/// RTF_UP from `<linux/route.h>` — the route is up.
const RTF_UP: u64 = 0x0001;

#[async_trait]
impl Probe for NetworkProbe {
    async fn snapshot(&self) -> Signals {
        Signals {
            has_connectivity: read_has_default_route(),
            ..Default::default()
        }
    }
}

/// Returns Some(true) when a usable default route exists, Some(false)
/// when /proc/net/route is readable but has no such row, None when
/// the file is missing / unreadable.
fn read_has_default_route() -> Option<bool> {
    let raw = std::fs::read_to_string(PROC_ROUTE).ok()?;
    let mut lines = raw.lines();
    // First line is the header; skip it.
    lines.next()?;
    for line in lines {
        let mut fields = line.split_whitespace();
        // Iface, Destination, Gateway, Flags, ...
        let _iface = fields.next()?;
        let dest = fields.next()?;
        let _gw = fields.next()?;
        let flags_hex = fields.next()?;
        if dest == "00000000" {
            let flags = u64::from_str_radix(flags_hex, 16).unwrap_or(0);
            if flags & RTF_UP != 0 {
                return Some(true);
            }
        }
    }
    Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn snapshot_returns_a_bool_on_linux() {
        let probe = NetworkProbe;
        let s = probe.snapshot().await;
        #[cfg(target_os = "linux")]
        {
            // On a Linux host /proc/net/route exists; it may be
            // empty (no routes) or populated — both are valid Some.
            assert!(s.has_connectivity.is_some());
        }
        let _ = s;
    }
}
