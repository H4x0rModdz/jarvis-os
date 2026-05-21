//! Concrete proactive rules + the UPower probe that feeds them.
//!
//! Two rules in V1, both battery:
//!
//!   - `battery_critical` — ≤5% discharging. 5-min cooldown so the
//!     user can dismiss without being bothered every tick. Urgency
//!     Critical so the shell can chime.
//!
//!   - `battery_low` — ≤15% discharging. 15-min cooldown. Urgency
//!     Warning.
//!
//! Idle rules are deferred — logind's IdleHint propagates only on
//! session-inactive transitions (swayidle's job), which is a
//! different signal than "user hasn't typed for 5 min". A future
//! commit will add a Wayland-portable idle source.

use crate::proactive::{BatteryState, EdgeRule, Nudge, Probe, Rule, Signals, Urgency};
use async_trait::async_trait;
use std::time::Duration;
use zbus::Connection;

/// Disk + memory rules. Same shape as `battery_rules` —
/// pure-function checks against `Signals`, per-rule cooldowns
/// sized so the user isn't pestered.
pub fn system_rules() -> Vec<Rule> {
    vec![
        Rule {
            name: "disk_critical",
            cooldown: Duration::from_secs(30 * 60),
            check: |s| {
                let pct = s.disk_root_free_pct?;
                if pct <= 5.0 {
                    Some(Nudge {
                        rule: "disk_critical",
                        text: format!(
                            "Disco quase cheio: só {pct:.0}% livre em /. \
                             Libere espaço antes que algo trave."
                        ),
                        urgency: Urgency::Critical,
                    })
                } else {
                    None
                }
            },
        },
        Rule {
            name: "disk_low",
            cooldown: Duration::from_secs(60 * 60),
            check: |s| {
                let pct = s.disk_root_free_pct?;
                if pct > 5.0 && pct <= 15.0 {
                    Some(Nudge {
                        rule: "disk_low",
                        text: format!(
                            "Disco com {pct:.0}% livre em /. \
                             Considere uma faxina."
                        ),
                        urgency: Urgency::Warning,
                    })
                } else {
                    None
                }
            },
        },
        Rule {
            name: "memory_critical",
            cooldown: Duration::from_secs(10 * 60),
            check: |s| {
                let mem_free = s.mem_free_pct?;
                // Swap is optional — when None we still warn on
                // pure RAM pressure to catch desktops without swap.
                let swap_used = s.swap_used_pct.unwrap_or(0.0);
                if mem_free < 5.0 && (s.swap_used_pct.is_none() || swap_used > 75.0) {
                    Some(Nudge {
                        rule: "memory_critical",
                        text: format!(
                            "Memória crítica: {mem_free:.0}% de RAM livre, \
                             swap em {swap_used:.0}%. Algum app pode ser \
                             morto a qualquer momento."
                        ),
                        urgency: Urgency::Critical,
                    })
                } else {
                    None
                }
            },
        },
        Rule {
            name: "memory_low",
            cooldown: Duration::from_secs(30 * 60),
            check: |s| {
                let mem_free = s.mem_free_pct?;
                // memory_critical also matches at <5%; carve memory_low
                // to (5%, 10%] so they don't both fire.
                if mem_free >= 5.0 && mem_free < 10.0 {
                    Some(Nudge {
                        rule: "memory_low",
                        text: format!(
                            "RAM apertada: {mem_free:.0}% livre. \
                             Fechar algumas abas do navegador ajuda."
                        ),
                        urgency: Urgency::Warning,
                    })
                } else {
                    None
                }
            },
        },
    ]
}

/// Edge-triggered network rules. Use `EdgeRule` so they only
/// fire on actual transitions — without that, the loop would
/// emit "wifi caiu" every 30 s while the user is offline.
///
/// Cooldown is short (60 s) on both — the only way they could
/// re-fire that quickly is genuine flapping, in which case the
/// user probably wants to know.
pub fn network_rules() -> Vec<EdgeRule> {
    vec![
        EdgeRule {
            name: "network_lost",
            cooldown: Duration::from_secs(60),
            check: |current, previous| {
                let prev = previous?;
                let prev_online = prev.has_connectivity?;
                let now_online = current.has_connectivity?;
                if prev_online && !now_online {
                    Some(Nudge {
                        rule: "network_lost",
                        text: "Sem internet. Verifique o Wi-Fi.".into(),
                        urgency: Urgency::Warning,
                    })
                } else {
                    None
                }
            },
        },
        EdgeRule {
            name: "network_restored",
            cooldown: Duration::from_secs(60),
            check: |current, previous| {
                let prev = previous?;
                let prev_online = prev.has_connectivity?;
                let now_online = current.has_connectivity?;
                if !prev_online && now_online {
                    Some(Nudge {
                        rule: "network_restored",
                        text: "Internet de volta.".into(),
                        urgency: Urgency::Info,
                    })
                } else {
                    None
                }
            },
        },
    ]
}

/// The two battery rules. `static` because Rule has function-pointer
/// `check` fields; building the Vec once at boot is enough.
pub fn battery_rules() -> Vec<Rule> {
    vec![
        Rule {
            name: "battery_critical",
            cooldown: Duration::from_secs(5 * 60),
            check: |s| {
                let pct = s.battery_percent?;
                let state = s.battery_state?;
                if state == BatteryState::Discharging && pct <= 5.0 {
                    Some(Nudge {
                        rule: "battery_critical",
                        text: format!(
                            "Bateria crítica em {pct:.0}%. Conecte o \
                             carregador agora pra não perder trabalho."
                        ),
                        urgency: Urgency::Critical,
                    })
                } else {
                    None
                }
            },
        },
        Rule {
            name: "battery_low",
            cooldown: Duration::from_secs(15 * 60),
            check: |s| {
                let pct = s.battery_percent?;
                let state = s.battery_state?;
                // Don't double-fire with battery_critical: the
                // critical rule covers ≤5%, low covers 6–15%.
                if state == BatteryState::Discharging && pct > 5.0 && pct <= 15.0 {
                    Some(Nudge {
                        rule: "battery_low",
                        text: format!(
                            "Bateria fraca em {pct:.0}%. Conecte o \
                             carregador quando puder."
                        ),
                        urgency: Urgency::Warning,
                    })
                } else {
                    None
                }
            },
        },
    ]
}

/// UPower DBus probe. Reads the synthesised DisplayDevice on the
/// system bus — same path the shell's PowerBridge subscribes to,
/// just queried on demand instead of via PropertiesChanged.
///
/// Failures are silent (returns empty Signals). The proactive
/// loop runs every 30 s; a transient DBus error on one tick is
/// not worth surfacing.
pub struct UPowerProbe;

const UPOWER_SERVICE: &str = "org.freedesktop.UPower";
const UPOWER_DEVICE_PATH: &str = "/org/freedesktop/UPower/devices/DisplayDevice";
const UPOWER_DEVICE_IFACE: &str = "org.freedesktop.UPower.Device";

#[async_trait]
impl Probe for UPowerProbe {
    async fn snapshot(&self) -> Signals {
        // Each query is independent — if UPower is down or the
        // DisplayDevice doesn't exist (desktop without battery),
        // we want partial answers, not all-or-nothing.
        let conn = match Connection::system().await {
            Ok(c) => c,
            Err(_) => return Signals::default(),
        };
        let percent = read_property::<f64>(&conn, "Percentage").await;
        let state_code = read_property::<u32>(&conn, "State").await;
        let type_code = read_property::<u32>(&conn, "Type").await;
        // UPower's Type=2 means Battery; anything else means there
        // isn't a laptop battery to talk about — skip the rule.
        let has_battery = type_code == Some(2);
        if !has_battery {
            return Signals::default();
        }
        Signals {
            battery_percent: percent,
            battery_state: state_code.map(decode_battery_state),
            idle_seconds: None,
            ..Default::default()
        }
    }
}

/// Map UPower's State enum. Same shape the shell's PowerBridge uses.
fn decode_battery_state(code: u32) -> BatteryState {
    match code {
        1 => BatteryState::Charging,
        2 => BatteryState::Discharging,
        4 => BatteryState::Full,
        _ => BatteryState::Unknown,
    }
}

async fn read_property<T>(conn: &Connection, name: &str) -> Option<T>
where
    T: TryFrom<zbus::zvariant::OwnedValue>,
{
    let proxy = zbus::fdo::PropertiesProxy::builder(conn)
        .destination(UPOWER_SERVICE)
        .ok()?
        .path(UPOWER_DEVICE_PATH)
        .ok()?
        .build()
        .await
        .ok()?;
    let value = proxy
        .get(UPOWER_DEVICE_IFACE.try_into().ok()?, name)
        .await
        .ok()?;
    // OwnedValue → T via TryFrom. zbus 4 provides impls for the
    // primitives we read (f64/u32/i64/String); `.ok()` drops the
    // associated Error so the bound doesn't need to name it.
    T::try_from(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proactive::{ProactiveEngine, Signals};

    fn signals(percent: f64, state: BatteryState) -> Signals {
        Signals {
            battery_percent: Some(percent),
            battery_state: Some(state),
            idle_seconds: None,
        }
    }

    #[test]
    fn battery_critical_fires_at_or_below_5() {
        let mut eng = ProactiveEngine::new(battery_rules());
        let nudges = eng.evaluate(&signals(4.0, BatteryState::Discharging));
        assert_eq!(nudges.len(), 1);
        assert_eq!(nudges[0].rule, "battery_critical");
        assert_eq!(nudges[0].urgency, Urgency::Critical);
        assert!(nudges[0].text.contains("4%"));
    }

    #[test]
    fn battery_low_fires_in_6_15_band() {
        let mut eng = ProactiveEngine::new(battery_rules());
        let nudges = eng.evaluate(&signals(12.0, BatteryState::Discharging));
        assert_eq!(nudges.len(), 1);
        assert_eq!(nudges[0].rule, "battery_low");
        assert_eq!(nudges[0].urgency, Urgency::Warning);
    }

    #[test]
    fn battery_critical_takes_priority_over_low() {
        // 5% — the critical bound is inclusive, the low bound starts
        // above 5%. Only critical should fire.
        let mut eng = ProactiveEngine::new(battery_rules());
        let nudges = eng.evaluate(&signals(5.0, BatteryState::Discharging));
        assert_eq!(nudges.len(), 1);
        assert_eq!(nudges[0].rule, "battery_critical");
    }

    #[test]
    fn no_nudge_when_charging() {
        let mut eng = ProactiveEngine::new(battery_rules());
        let nudges = eng.evaluate(&signals(3.0, BatteryState::Charging));
        assert!(nudges.is_empty());
    }

    #[test]
    fn no_nudge_without_battery_signal() {
        let mut eng = ProactiveEngine::new(battery_rules());
        let nudges = eng.evaluate(&Signals::default());
        assert!(nudges.is_empty());
    }

    #[test]
    fn healthy_charge_no_nudge() {
        let mut eng = ProactiveEngine::new(battery_rules());
        let nudges = eng.evaluate(&signals(80.0, BatteryState::Discharging));
        assert!(nudges.is_empty());
    }

    // ── system rules ───────────────────────────────────────────────

    fn disk_only(pct: f64) -> Signals {
        Signals {
            disk_root_free_pct: Some(pct),
            ..Default::default()
        }
    }

    fn mem_only(free_pct: f64, swap_used: Option<f64>) -> Signals {
        Signals {
            mem_free_pct: Some(free_pct),
            swap_used_pct: swap_used,
            ..Default::default()
        }
    }

    #[test]
    fn disk_critical_fires_at_or_below_5() {
        let mut eng = ProactiveEngine::new(system_rules());
        let nudges = eng.evaluate(&disk_only(3.0));
        assert_eq!(nudges.len(), 1);
        assert_eq!(nudges[0].rule, "disk_critical");
        assert_eq!(nudges[0].urgency, Urgency::Critical);
    }

    #[test]
    fn disk_low_fires_in_6_15_band() {
        let mut eng = ProactiveEngine::new(system_rules());
        let nudges = eng.evaluate(&disk_only(10.0));
        assert_eq!(nudges.len(), 1);
        assert_eq!(nudges[0].rule, "disk_low");
        assert_eq!(nudges[0].urgency, Urgency::Warning);
    }

    #[test]
    fn disk_critical_takes_priority_at_5_pct() {
        let mut eng = ProactiveEngine::new(system_rules());
        let nudges = eng.evaluate(&disk_only(5.0));
        assert_eq!(nudges.len(), 1);
        assert_eq!(nudges[0].rule, "disk_critical");
    }

    #[test]
    fn disk_healthy_no_nudge() {
        let mut eng = ProactiveEngine::new(system_rules());
        let nudges = eng.evaluate(&disk_only(80.0));
        assert!(nudges.is_empty());
    }

    #[test]
    fn memory_critical_fires_when_low_ram_and_swap_pressured() {
        let mut eng = ProactiveEngine::new(system_rules());
        let nudges = eng.evaluate(&mem_only(3.0, Some(80.0)));
        let critical: Vec<_> = nudges
            .iter()
            .filter(|n| n.rule == "memory_critical")
            .collect();
        assert_eq!(critical.len(), 1);
        assert_eq!(critical[0].urgency, Urgency::Critical);
    }

    #[test]
    fn memory_critical_fires_when_swap_absent_and_ram_below_5() {
        // No swap configured (None) — ram alone is enough to fire.
        let mut eng = ProactiveEngine::new(system_rules());
        let nudges = eng.evaluate(&mem_only(3.0, None));
        assert!(nudges.iter().any(|n| n.rule == "memory_critical"));
    }

    #[test]
    fn memory_low_fires_in_5_10_band() {
        let mut eng = ProactiveEngine::new(system_rules());
        let nudges = eng.evaluate(&mem_only(8.0, Some(20.0)));
        let low: Vec<_> = nudges.iter().filter(|n| n.rule == "memory_low").collect();
        assert_eq!(low.len(), 1);
        assert_eq!(low[0].urgency, Urgency::Warning);
    }

    #[test]
    fn memory_low_does_not_fire_when_critical_also_matches() {
        // mem=3% → critical hits; low band starts at 5% so it
        // should NOT match. memory_critical exclusive at <5%.
        let mut eng = ProactiveEngine::new(system_rules());
        let nudges = eng.evaluate(&mem_only(3.0, Some(80.0)));
        let low: Vec<_> = nudges.iter().filter(|n| n.rule == "memory_low").collect();
        assert!(
            low.is_empty(),
            "memory_low must not double-fire with critical"
        );
    }

    #[test]
    fn memory_healthy_no_nudge() {
        let mut eng = ProactiveEngine::new(system_rules());
        let nudges = eng.evaluate(&mem_only(60.0, Some(10.0)));
        assert!(nudges.is_empty());
    }

    // ── network rules (edge-triggered) ─────────────────────────────

    fn conn(has_internet: bool) -> Signals {
        Signals {
            has_connectivity: Some(has_internet),
            ..Default::default()
        }
    }

    #[test]
    fn network_lost_fires_on_online_to_offline_transition() {
        let mut eng = ProactiveEngine::with_edge_rules(vec![], network_rules());
        // Tick 1: online (warms the prev_signals).
        let warm = eng.evaluate(&conn(true));
        assert!(warm.is_empty());
        // Tick 2: offline → fire network_lost.
        let nudges = eng.evaluate(&conn(false));
        assert_eq!(nudges.len(), 1);
        assert_eq!(nudges[0].rule, "network_lost");
        assert_eq!(nudges[0].urgency, Urgency::Warning);
    }

    #[test]
    fn network_restored_fires_on_offline_to_online_transition() {
        let mut eng = ProactiveEngine::with_edge_rules(vec![], network_rules());
        eng.evaluate(&conn(false)); // warm
        let nudges = eng.evaluate(&conn(true));
        assert_eq!(nudges.len(), 1);
        assert_eq!(nudges[0].rule, "network_restored");
        assert_eq!(nudges[0].urgency, Urgency::Info);
    }

    #[test]
    fn no_network_nudge_on_first_tick() {
        // Booting offline shouldn't fire — we never saw a transition.
        let mut eng = ProactiveEngine::with_edge_rules(vec![], network_rules());
        let nudges = eng.evaluate(&conn(false));
        assert!(nudges.is_empty());
    }

    #[test]
    fn no_network_nudge_on_steady_state() {
        let mut eng = ProactiveEngine::with_edge_rules(vec![], network_rules());
        eng.evaluate(&conn(true));
        let nudges = eng.evaluate(&conn(true));
        assert!(nudges.is_empty(), "online → online is no transition");
    }
}
