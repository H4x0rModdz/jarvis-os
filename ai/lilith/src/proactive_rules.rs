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

use crate::proactive::{BatteryState, Nudge, Probe, Rule, Signals, Urgency};
use async_trait::async_trait;
use std::time::Duration;
use zbus::Connection;

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
    T: zbus::zvariant::Type
        + for<'de> serde::de::Deserialize<'de>
        + std::fmt::Debug
        + 'static,
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
    // OwnedValue → T via try_from. zbus picks the right conversion
    // when the inner variant matches T.
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
}
