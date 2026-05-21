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
    /// True when the host has a default route (i.e. can talk to
    /// anything outside its own LAN). None when /proc/net/route
    /// is unreadable.
    pub has_connectivity: Option<bool>,
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
        if other.has_connectivity.is_some() {
            self.has_connectivity = other.has_connectivity;
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

/// One stateless proactive rule — fires from a single Signals
/// snapshot. Battery / disk / memory rules are all this shape.
pub struct Rule {
    pub name: &'static str,
    pub cooldown: Duration,
    pub check: fn(&Signals) -> Option<Nudge>,
}

/// One edge-triggered rule — fires on a transition between two
/// snapshots. Network rules use this so "wifi caiu" only toasts
/// when connectivity goes online → offline, not on every tick
/// while offline. The `previous` argument is `None` on the first
/// tick after daemon start (no prior snapshot yet); edge rules
/// typically refuse to fire in that case so a fresh boot doesn't
/// generate a "lost!" toast for a state we never observed.
pub struct EdgeRule {
    pub name: &'static str,
    pub cooldown: Duration,
    pub check: fn(&Signals, Option<&Signals>) -> Option<Nudge>,
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

/// In-memory engine. Owns the rule table + the cooldown map +
/// the previous-signals snapshot for edge rules. Callers tick by
/// calling `evaluate` periodically — the engine itself doesn't
/// spawn a task; main.rs schedules.
pub struct ProactiveEngine {
    rules: Vec<Rule>,
    edge_rules: Vec<EdgeRule>,
    last_fired: HashMap<&'static str, Instant>,
    prev_signals: Option<Signals>,
}

impl ProactiveEngine {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self {
            rules,
            edge_rules: Vec::new(),
            last_fired: HashMap::new(),
            prev_signals: None,
        }
    }

    /// Construct an engine that runs both stateless + edge rules.
    /// Both lists are evaluated each tick; stateless first, then
    /// edge. Cooldowns are tracked per-rule across both kinds.
    pub fn with_edge_rules(rules: Vec<Rule>, edge_rules: Vec<EdgeRule>) -> Self {
        Self {
            rules,
            edge_rules,
            last_fired: HashMap::new(),
            prev_signals: None,
        }
    }

    /// Evaluate every rule against `signals` and return the nudges
    /// that fire this tick. Each fire updates the cooldown stamp,
    /// so calling `evaluate` twice in a row only emits a given rule
    /// once.
    ///
    /// Edge rules also see the previous-tick snapshot; on the very
    /// first tick `previous` is `None` and edge rules typically
    /// refuse to fire to avoid alarming on a state we never saw a
    /// transition into.
    pub fn evaluate(&mut self, signals: &Signals) -> Vec<Nudge> {
        let now = Instant::now();
        let mut out = Vec::new();
        let mut fired: Vec<(&'static str, Nudge)> = Vec::new();

        // ── Stateless rules ───────────────────────────────────────
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
        // ── Edge rules ────────────────────────────────────────────
        for rule in &self.edge_rules {
            if let Some(prev) = self.last_fired.get(rule.name) {
                if now.duration_since(*prev) < rule.cooldown {
                    continue;
                }
            }
            if let Some(nudge) = (rule.check)(signals, self.prev_signals.as_ref()) {
                fired.push((rule.name, nudge));
            }
        }
        for (name, nudge) in fired {
            self.last_fired.insert(name, now);
            out.push(nudge);
        }

        // Persist this tick's snapshot for the next call's edge
        // comparison. Cloned because Signals is small + edge rules
        // shouldn't be affected by later mutations of the live
        // probe outputs.
        self.prev_signals = Some(signals.clone());

        out
    }

    #[cfg(test)]
    pub fn rule_count(&self) -> usize {
        self.rules.len() + self.edge_rules.len()
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

    #[tokio::test]
    async fn composite_probe_merges_fields_from_multiple_probes() {
        // Two probes, each filling disjoint fields. CompositeProbe
        // should return a Signals with both populated.
        struct BatteryOnly;
        #[async_trait]
        impl Probe for BatteryOnly {
            async fn snapshot(&self) -> Signals {
                Signals {
                    battery_percent: Some(42.0),
                    battery_state: Some(BatteryState::Discharging),
                    ..Default::default()
                }
            }
        }
        struct DiskOnly;
        #[async_trait]
        impl Probe for DiskOnly {
            async fn snapshot(&self) -> Signals {
                Signals {
                    disk_root_free_pct: Some(33.0),
                    mem_free_pct: Some(50.0),
                    ..Default::default()
                }
            }
        }

        let composite = CompositeProbe::new(vec![
            std::sync::Arc::new(BatteryOnly),
            std::sync::Arc::new(DiskOnly),
        ]);
        let s = composite.snapshot().await;
        assert_eq!(s.battery_percent, Some(42.0));
        assert_eq!(s.battery_state, Some(BatteryState::Discharging));
        assert_eq!(s.disk_root_free_pct, Some(33.0));
        assert_eq!(s.mem_free_pct, Some(50.0));
    }

    fn edge_transition_rule() -> EdgeRule {
        EdgeRule {
            name: "wifi_lost",
            cooldown: Duration::from_secs(60),
            check: |current, previous| {
                let prev = previous?;
                let prev_online = prev.has_connectivity?;
                let now_online = current.has_connectivity?;
                if prev_online && !now_online {
                    Some(Nudge {
                        rule: "wifi_lost",
                        text: "wifi caiu".into(),
                        urgency: Urgency::Warning,
                    })
                } else {
                    None
                }
            },
        }
    }

    #[test]
    fn edge_rule_does_not_fire_on_first_tick() {
        // First call: prev is None — transition rule should NOT fire
        // (no prior state to transition from).
        let mut eng =
            ProactiveEngine::with_edge_rules(vec![], vec![edge_transition_rule()]);
        let s = Signals {
            has_connectivity: Some(false),
            ..Default::default()
        };
        let nudges = eng.evaluate(&s);
        assert!(nudges.is_empty(), "should not fire on first tick");
    }

    #[test]
    fn edge_rule_fires_on_true_to_false_transition() {
        let mut eng =
            ProactiveEngine::with_edge_rules(vec![], vec![edge_transition_rule()]);
        // Tick 1: online.
        let online = Signals {
            has_connectivity: Some(true),
            ..Default::default()
        };
        eng.evaluate(&online);
        // Tick 2: offline. Transition true→false → fire.
        let offline = Signals {
            has_connectivity: Some(false),
            ..Default::default()
        };
        let nudges = eng.evaluate(&offline);
        assert_eq!(nudges.len(), 1);
        assert_eq!(nudges[0].rule, "wifi_lost");
    }

    #[test]
    fn edge_rule_no_fire_when_steady_state() {
        let mut eng =
            ProactiveEngine::with_edge_rules(vec![], vec![edge_transition_rule()]);
        let offline = Signals {
            has_connectivity: Some(false),
            ..Default::default()
        };
        eng.evaluate(&offline); // first tick — no prev
        // Second tick still offline; no transition.
        let nudges = eng.evaluate(&offline);
        assert!(nudges.is_empty(), "no transition → no fire");
    }

    #[tokio::test]
    async fn composite_probe_later_probe_overrides_earlier() {
        // When two probes write the same field, last one wins per
        // the merge contract.
        struct First;
        #[async_trait]
        impl Probe for First {
            async fn snapshot(&self) -> Signals {
                Signals {
                    mem_free_pct: Some(10.0),
                    ..Default::default()
                }
            }
        }
        struct Second;
        #[async_trait]
        impl Probe for Second {
            async fn snapshot(&self) -> Signals {
                Signals {
                    mem_free_pct: Some(90.0),
                    ..Default::default()
                }
            }
        }

        let composite =
            CompositeProbe::new(vec![std::sync::Arc::new(First), std::sync::Arc::new(Second)]);
        let s = composite.snapshot().await;
        assert_eq!(s.mem_free_pct, Some(90.0), "later probe should win");
    }
}
