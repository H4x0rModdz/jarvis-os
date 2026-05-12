//! Jarvis Notifications daemon.
//!
//! Holds the `org.freedesktop.Notifications` well-known name (so every
//! `notify-send` lands here) and our own `com.jarvis.Notifications`
//! interface (so the shell can render in our style + Lilith can read
//! the recent history).
//!
//! See `system/notifications/module.md` for the contract and ADR 0010
//! for the rationale.

use chrono::Utc;
use serde::Serialize;
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use zbus::{connection, interface, SignalContext};
use zvariant::Value as ZValue;

/// How many notifications we keep in the in-memory history. Picked low
/// enough to be cheap; high enough that "show me everything from the
/// last hour" is plausible.
const HISTORY_CAPACITY: usize = 64;

/// FreeDesktop urgency hint values: 0=low, 1=normal, 2=critical.
fn urgency_from_byte(b: u8) -> &'static str {
    match b {
        0 => "low",
        2 => "critical",
        _ => "normal",
    }
}

#[derive(Debug, Clone, Serialize)]
struct Entry {
    id: u32,
    app: String,
    summary: String,
    body: String,
    urgency: String,
    posted_at: String,
    /// FreeDesktop `actions` array, alternating key + display label.
    /// V1 ignored this; V2 stores it so the shell can render buttons
    /// and call `InvokeAction` back when the user clicks one.
    #[serde(default)]
    actions: Vec<String>,
}

struct Service {
    next_id: AsyncMutex<u32>,
    /// Shared with the `History` interface served on a sibling DBus
    /// path — same `Vec` is both written from `Notify()` and read from
    /// `RecentNotifications()`.
    history: Arc<AsyncMutex<VecDeque<Entry>>>,
}

#[interface(name = "org.freedesktop.Notifications")]
impl Service {
    /// FreeDesktop spec entry point. V2 honours `actions` (key/label
    /// pairs the shell renders as buttons) and emits `ActionInvoked`
    /// when the user clicks one. Image hints still deferred.
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        _app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<String>,
        hints: HashMap<String, ZValue<'_>>,
        _expire_timeout: i32,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> u32 {
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            let mut next = self.next_id.lock().await;
            *next = next.wrapping_add(1).max(1);
            *next
        };

        let urgency = hints
            .get("urgency")
            .and_then(|v| u8::try_from(v).ok())
            .map(urgency_from_byte)
            .unwrap_or("normal");

        let entry = Entry {
            id,
            app: app_name.to_string(),
            summary: summary.to_string(),
            body: body.to_string(),
            urgency: urgency.to_string(),
            posted_at: Utc::now().to_rfc3339(),
            actions: actions.clone(),
        };

        // Keep history bounded.
        {
            let mut h = self.history.lock().await;
            // Replace-by-id when replaces_id was provided.
            if replaces_id != 0 {
                h.retain(|e| e.id != replaces_id);
            }
            if h.len() == HISTORY_CAPACITY {
                h.pop_front();
            }
            h.push_back(entry.clone());
        }

        tracing::info!(id, app = %app_name, summary, urgency, "Notification posted");

        if let Err(e) =
            Self::notification_posted(&ctx, id, app_name, summary, body, urgency, &actions).await
        {
            tracing::warn!("NotificationPosted emit failed: {e}");
        }

        id
    }

    /// Called by the shell when the user clicks one of the action
    /// buttons on a toast. Re-emits as the FreeDesktop spec's
    /// `ActionInvoked` signal so the originating app sees its
    /// callback fire.
    async fn invoke_action(
        &self,
        id: u32,
        action_key: &str,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) {
        tracing::info!(id, action_key, "Action invoked");
        if let Err(e) = Self::action_invoked(&ctx, id, action_key).await {
            tracing::warn!("ActionInvoked emit failed: {e}");
        }
    }

    async fn close_notification(
        &self,
        id: u32,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> () {
        let removed = {
            let mut h = self.history.lock().await;
            let before = h.len();
            h.retain(|e| e.id != id);
            before != h.len()
        };

        if removed {
            if let Err(e) = Self::notification_closed(&ctx, id, 3 /* closed by call */).await {
                tracing::warn!("NotificationClosed emit failed: {e}");
            }
        }
    }

    /// What this server supports. `body-markup` lets apps put basic
    /// HTML-ish formatting in the body; `actions` enables the button
    /// row added in V2.
    async fn get_capabilities(&self) -> Vec<String> {
        vec![
            "body".into(),
            "body-markup".into(),
            "persistence".into(),
            "actions".into(),
        ]
    }

    async fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Jarvis Notifications".into(),
            "Jarvis OS".into(),
            env!("CARGO_PKG_VERSION").into(),
            "1.2".into(),
        )
    }

    #[zbus(signal)]
    async fn notification_closed(ctx: &SignalContext<'_>, id: u32, reason: u32)
        -> zbus::Result<()>;

    /// FreeDesktop spec signal — fires when the user clicks an action
    /// button on the toast. The originating app receives this and
    /// dispatches whatever the action key was supposed to trigger.
    #[zbus(signal)]
    async fn action_invoked(ctx: &SignalContext<'_>, id: u32, action_key: &str)
        -> zbus::Result<()>;

    /// Re-emitted Notify() — this is what the shell subscribes to so
    /// it doesn't have to implement the spec itself. V2 adds the
    /// `actions` slice so the toast can render its button row without
    /// a separate roundtrip.
    #[zbus(signal)]
    async fn notification_posted(
        ctx: &SignalContext<'_>,
        id: u32,
        app: &str,
        summary: &str,
        body: &str,
        urgency: &str,
        actions: &[String],
    ) -> zbus::Result<()>;
}

/// Jarvis-private slice of the same Service object — same DBus name
/// can't be used twice, so we put RecentNotifications on a different
/// path/interface served by the same struct.
struct History {
    history: Arc<AsyncMutex<VecDeque<Entry>>>,
}

#[interface(name = "com.jarvis.Notifications")]
impl History {
    /// Returns the last `limit` notifications (or every entry if
    /// `limit == 0`), oldest first. Serialised as JSON for parity with
    /// the rest of the Jarvis daemons.
    async fn recent_notifications(&self, limit: u32) -> String {
        let snapshot: Vec<Entry> = {
            let h = self.history.lock().await;
            if limit == 0 {
                h.iter().cloned().collect()
            } else {
                let take = limit as usize;
                let skip = h.len().saturating_sub(take);
                h.iter().skip(skip).cloned().collect()
            }
        };
        json!(snapshot).to_string()
    }

    /// Drop one entry from the history buffer. Called by the shell
    /// when the user clicks the × on a row in the drawer. Distinct
    /// from FreeDesktop's `CloseNotification` because that signals
    /// the originating app (reason=3) — Dismiss is purely a UI
    /// concern, the app doesn't need to know.
    async fn dismiss(
        &self,
        id: u32,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> bool {
        let removed = {
            let mut h = self.history.lock().await;
            let before = h.len();
            h.retain(|e| e.id != id);
            before != h.len()
        };
        if removed {
            if let Err(e) = Self::history_changed(&ctx).await {
                tracing::warn!("HistoryChanged emit failed: {e}");
            }
        }
        removed
    }

    /// Wipe every entry. Triggered by the drawer's "Clear all"
    /// button — same UI-only semantics as Dismiss.
    async fn clear(&self, #[zbus(signal_context)] ctx: SignalContext<'_>) -> u32 {
        let cleared = {
            let mut h = self.history.lock().await;
            let n = h.len() as u32;
            h.clear();
            n
        };
        if cleared > 0 {
            if let Err(e) = Self::history_changed(&ctx).await {
                tracing::warn!("HistoryChanged emit failed: {e}");
            }
        }
        cleared
    }

    /// Fires after Dismiss / Clear so the shell knows to re-pull
    /// the list rather than polling.
    #[zbus(signal)]
    async fn history_changed(ctx: &SignalContext<'_>) -> zbus::Result<()>;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jarvis_notifications=info".parse()?),
        )
        .init();

    tracing::info!("Starting Jarvis Notifications");

    let history = Arc::new(AsyncMutex::new(VecDeque::with_capacity(HISTORY_CAPACITY)));
    let service = Service {
        next_id: AsyncMutex::new(0),
        history: history.clone(),
    };
    let history_iface = History {
        history: history.clone(),
    };

    let _conn = connection::Builder::session()?
        .name("org.freedesktop.Notifications")?
        .serve_at("/org/freedesktop/Notifications", service)?
        .name("com.jarvis.Notifications")?
        .serve_at("/com/jarvis/Notifications", history_iface)?
        .build()
        .await?;

    tracing::info!(
        "Notifications ready on org.freedesktop.Notifications + com.jarvis.Notifications"
    );

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urgency_byte_known_values() {
        assert_eq!(urgency_from_byte(0), "low");
        assert_eq!(urgency_from_byte(1), "normal");
        assert_eq!(urgency_from_byte(2), "critical");
        assert_eq!(urgency_from_byte(7), "normal"); // unknown -> normal
    }

    #[tokio::test]
    async fn history_evicts_oldest_at_capacity() {
        let h: VecDeque<Entry> = (0..HISTORY_CAPACITY as u32)
            .map(|i| Entry {
                id: i + 1,
                app: format!("app{i}"),
                summary: "".into(),
                body: "".into(),
                urgency: "normal".into(),
                posted_at: "".into(),
                actions: vec![],
            })
            .collect();
        let buf = Arc::new(AsyncMutex::new(h));

        // Push one more — should evict the first entry.
        {
            let mut lock = buf.lock().await;
            if lock.len() == HISTORY_CAPACITY {
                lock.pop_front();
            }
            lock.push_back(Entry {
                id: 999,
                app: "new".into(),
                summary: "".into(),
                body: "".into(),
                urgency: "normal".into(),
                posted_at: "".into(),
                actions: vec![],
            });
        }

        let lock = buf.lock().await;
        assert_eq!(lock.len(), HISTORY_CAPACITY);
        assert_eq!(lock.front().unwrap().id, 2);
        assert_eq!(lock.back().unwrap().id, 999);
    }
}
