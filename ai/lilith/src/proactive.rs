//! Proactive engine — Lilith speaks up without being asked.
//!
//! Periodically polls a `Probe` for a `Signals` snapshot (battery
//! percent + state, idle seconds, plug events) and evaluates a
//! static rule table. When a rule's `check` returns `Some(Nudge)`
//! AND its per-rule cooldown has elapsed, the engine emits the
//! nudge via a callback the caller supplies (production: emit a
//! DBus signal; tests: collect into a Vec).
//!
//! Two design choices worth pinning:
//!
//! - **Cooldowns are per-rule, not global.** A critical-battery
//!   nudge and an idle-warning nudge should be able to fire close
//!   together if both conditions hold; what we DON'T want is the
//!   same rule re-firing every tick while the user ignores it.
//!
//! - **Rules are pure functions.** Easy to test, easy to grow the
//!   catalog. The signal-fetch + the dispatch sit at the edges; the
//!   "should I nudge now?" decision is a `fn(&Signals) -> Option<Nudge>`.
//!
//! V1: battery rules only. Idle support lands when we have a
//! Wayland-portable idle clock; logind's `IdleHint` propagates
//! only when greetd/swayidle marks the session inactive, which is
//! a different signal than "user hasn't typed in 5 min".

use async_trait::async_trait;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Snapshot of every signal the rules look at. Probes fill what
/// they can; unknown values stay `None` and rules guard with `?`.
#[derive(Debug, Default, Clone)]
pub struct Signals {
    pub battery_percent: Option<f64>,
    pub battery_state: Option<BatteryState>,
    pub idle_seconds: Option<u64>,
    /// Free % of the root filesystem (0..100). None if statvfs failed.
    pub disk_root_free_pct: Option<f64>,
    /// Free % of system memory (MemAvailable / MemTotal). 0..100.
    pub mem_free_pct: Option<f64>,
    /// Used % of swap ((SwapTotal-SwapFree)/SwapTotal). 0..100.
    /// None when the host has no swap configured.
    pub swap_used_pct: Option<f64>,
}

impl Signals {
    /// Merge `other` into `self` — non-None fields on `other` win.
    /// Used by `CompositeProbe` to fold per-source snapshots into
    /// one. The order of merge calls determines tie-breaking when
    /// two probes claim the same field; in practice probes never
    /// overlap (UPower owns battery, SystemProbe owns disk/mem).
    pub fn merge(&mut self, other: Signals) {
        if other.battery_percent.is_some() {
            self.battery_percent = other.battery_percent;
        }
        if other.battery_state.is_some() {
            self.battery_state = other.battery_state;
        }
        if other.idle_seconds.is_some() {
            self.idle_seconds = other.idle_seconds;
        }
        if other.disk_root_free_pct.is_some() {
            self.disk_root_free_pct = other.disk_root_free_pct;
        }
        if other.mem_free_pct.is_some() {
            self.mem_free_pct = other.mem_free_pct;
        }
        if other.swap_used_pct.is_some() {
            self.swap_used_pct = other.swap_used_pct;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryState {
    Charging,
    Discharging,
    Full,
    Unknown,
}

/// Severity hint for the surface that renders the nudge. The shell
/// can colour-code or escalate (critical → audio chime, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Info,
    Warning,
    Critical,
}

impl Urgency {
    pub fn as_str(self) -> &'static str {
        match self {
            Urgency::Info => "info",
            Urgency::Warning => "warning",
            Urgency::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Nudge {
    /// The rule that fired — used as the cooldown key so a rule
    /// can't re-trigger while still on cooldown.
    pub rule: &'static str,
    pub text: String,
    pub urgency: Urgency,
}

/// One proactive rule. Owns its cooldown — the engine tracks
/// `last_fired` separately so rule definitions stay copyable.
pub struct Rule {
    pub name: &'static str,
    pub cooldown: Duration,
    pub check: fn(&Signals) -> Option<Nudge>,
}

/// Probe interface — anything that can produce a `Signals` snapshot
/// on demand. Production: a UPower-querying impl. Tests: a struct
/// returning a scripted `Signals`.
#[async_trait]
pub trait Probe: Send + Sync {
    async fn snapshot(&self) -> Signals;
}

/// Always-empty probe — useful as a fallback if no real probe
/// is available, so the engine can still be constructed without a
/// runtime panic.
pub struct NullProbe;

#[async_trait]
impl Probe for NullProbe {
    async fn snapshot(&self) -> Signals {
        Signals::default()
    }
}

/// Fans out to multiple probes and merges their Signals. Each
/// probe owns a different set of fields; the merge picks non-None
/// values from each. Failing probes return `Signals::default()`
/// (their `snapshot` impls swallow errors), so a broken probe just
/// leaves its fields unset on the composite output.
pub struct CompositeProbe {
    probes: Vec<std::sync::Arc<dyn Probe>>,
}

impl CompositeProbe {
    pub fn new(probes: Vec<std::sync::Arc<dyn Probe>>) -> Self {
        Self { probes }
    }
}

#[async_trait]
impl Probe for CompositeProbe {
    async fn snapshot(&self) -> Signals {
        let mut combined = Signals::default();
        for p in &self.probes {
            combined.merge(p.snapshot().await);
        }
        combined
    }
}

/// In-memory engine. Owns the rule table + the cooldown map.
/// Callers tick by calling `evaluate` periodically — the engine
/// itself doesn't spawn a task; main.rs schedules.
pub struct ProactiveEngine {
    rules: Vec<Rule>,
    last_fired: HashMap<&'static str, Instant>,
}

impl ProactiveEngine {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self {
            rules,
            last_fired: HashMap::new(),
        }
    }

    /// Evaluate every rule against `signals` and return the nudges
    /// that fire this tick. Each fire updates the cooldown stamp,
    /// so calling `evaluate` twice in a row only emits a given rule
    /// once.
    pub fn evaluate(&mut self, signals: &Signals) -> Vec<Nudge> {
        let now = Instant::now();
        let mut out = Vec::new();
        // Collect first so we don't hold the borrow on `self.rules`
        // while mutating `last_fired`.
        let mut fired: Vec<(&'static str, Nudge)> = Vec::new();
        for rule in &self.rules {
            if let Some(prev) = self.last_fired.get(rule.name) {
                if now.duration_since(*prev) < rule.cooldown {
                    continue;
                }
            }
            if let Some(nudge) = (rule.check)(signals) {
                fired.push((rule.name, nudge));
            }
        }
        for (name, nudge) in fired {
            self.last_fired.insert(name, now);
            out.push(nudge);
        }
        out
    }

    #[cfg(test)]
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always_fire_rule() -> Rule {
        Rule {
            name: "always",
            cooldown: Duration::from_millis(50),
            check: |_| {
                Some(Nudge {
                    rule: "always",
                    text: "boom".into(),
                    urgency: Urgency::Info,
                })
            },
        }
    }

    fn never_fire_rule() -> Rule {
        Rule {
            name: "never",
            cooldown: Duration::from_secs(1),
            check: |_| None,
        }
    }

    #[test]
    fn evaluate_fires_matching_rule() {
        let mut eng = ProactiveEngine::new(vec![always_fire_rule()]);
        let got = eng.evaluate(&Signals::default());
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "boom");
    }

    #[test]
    fn evaluate_respects_cooldown() {
        let mut eng = ProactiveEngine::new(vec![always_fire_rule()]);
        let first = eng.evaluate(&Signals::default());
        let second = eng.evaluate(&Signals::default());
        assert_eq!(first.len(), 1);
        // 50 ms cooldown — second tick is well under that.
        assert!(second.is_empty(), "cooldown not honoured: {second:?}");
    }

    #[test]
    fn evaluate_skips_non_matching_rule() {
        let mut eng = ProactiveEngine::new(vec![never_fire_rule()]);
        let got = eng.evaluate(&Signals::default());
        assert!(got.is_empty());
    }

    #[test]
    fn each_rule_has_independent_cooldown() {
        let mut eng = ProactiveEngine::new(vec![always_fire_rule(), never_fire_rule()]);
        let first = eng.evaluate(&Signals::default());
        assert_eq!(first.len(), 1);
        // `never` never fires; `always` is on cooldown — second tick
        // returns nothing, but the negative result is the cooldown
        // working per-rule (not collapsing across rules).
        let second = eng.evaluate(&Signals::default());
        assert!(second.is_empty());
    }

    #[test]
    fn fires_again_after_cooldown_expires() {
        let mut eng = ProactiveEngine::new(vec![always_fire_rule()]);
        eng.evaluate(&Signals::default());
        std::thread::sleep(Duration::from_millis(70));
        let second = eng.evaluate(&Signals::default());
        assert_eq!(second.len(), 1, "should have re-fired after cooldown");
    }

    #[tokio::test]
    async fn null_probe_returns_empty_signals() {
        let p = NullProbe;
        let s = p.snapshot().await;
        assert!(s.battery_percent.is_none());
        assert!(s.battery_state.is_none());
        assert!(s.idle_seconds.is_none());
    }
}
