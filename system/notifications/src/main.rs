//! Jarvis Notifications daemon.
//!
//! Holds the `org.freedesktop.Notifications` well-known name (so every
//! `notify-send` lands here) and our own `com.jarvis.Notifications`
//! interface (so the shell can render in our style + Lilith can read
//! the recent history).
//!
//! V3 — history is persisted to `~/.jarvis/notifications.db` via
//! SQLite. Survives daemon restarts; capped at 500 rows with
//! oldest-first eviction. See `module.md` and ADR 0010.

mod store;

use anyhow::Context;
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use store::{Entry, HistoryStore};
use tokio::sync::Mutex as AsyncMutex;
use zbus::{connection, interface, SignalContext};
use zvariant::Value as ZValue;

/// FreeDesktop urgency hint values: 0=low, 1=normal, 2=critical.
fn urgency_from_byte(b: u8) -> &'static str {
    match b {
        0 => "low",
        2 => "critical",
        _ => "normal",
    }
}

struct Service {
    next_id: AsyncMutex<u32>,
    /// SQLite-backed history. Same store is shared with the `History`
    /// interface served on a sibling DBus path; both interfaces are
    /// served by separate structs that hold their own clone of the
    /// Arc, because zbus can't serve the same struct under two
    /// interfaces on the same connection.
    history: Arc<HistoryStore>,
}

#[interface(name = "org.freedesktop.Notifications")]
impl Service {
    /// FreeDesktop spec entry point. Honours `actions` (key/label
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

        // INSERT OR REPLACE — replaces_id semantics fall out for free.
        if let Err(e) = self.history.insert(&entry) {
            tracing::warn!(error = %e, "Notification persist failed");
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

    async fn close_notification(&self, id: u32, #[zbus(signal_context)] ctx: SignalContext<'_>) {
        let removed = self.history.dismiss(id).unwrap_or(false);
        if removed {
            if let Err(e) = Self::notification_closed(&ctx, id, 3 /* closed by call */).await {
                tracing::warn!("NotificationClosed emit failed: {e}");
            }
        }
    }

    /// What this server supports. `body-markup` lets apps put basic
    /// HTML-ish formatting in the body; `actions` enables the button
    /// row added in V2; `persistence` becomes meaningful in V3 (we
    /// actually survive a daemon restart now).
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

    #[zbus(signal)]
    async fn action_invoked(ctx: &SignalContext<'_>, id: u32, action_key: &str)
        -> zbus::Result<()>;

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
    history: Arc<HistoryStore>,
}

#[interface(name = "com.jarvis.Notifications")]
impl History {
    /// Returns the last `limit` notifications (or every entry if
    /// `limit == 0`), oldest first. Serialised as JSON for parity with
    /// the rest of the Jarvis daemons.
    async fn recent_notifications(&self, limit: u32) -> String {
        let snapshot = self.history.recent(limit).unwrap_or_default();
        // Reuse the store::Entry serializer; it matches what the V2
        // client (the shell drawer) already expects.
        json!(snapshot
            .iter()
            .map(|e| json!({
                "id": e.id,
                "app": e.app,
                "summary": e.summary,
                "body": e.body,
                "urgency": e.urgency,
                "posted_at": e.posted_at,
                "actions": e.actions,
            }))
            .collect::<Vec<_>>())
        .to_string()
    }

    /// Drop one entry from the history buffer. UI-only — distinct
    /// from FreeDesktop's `CloseNotification` which signals the
    /// originating app.
    async fn dismiss(&self, id: u32, #[zbus(signal_context)] ctx: SignalContext<'_>) -> bool {
        let removed = self.history.dismiss(id).unwrap_or(false);
        if removed {
            if let Err(e) = Self::history_changed(&ctx).await {
                tracing::warn!("HistoryChanged emit failed: {e}");
            }
        }
        removed
    }

    /// Wipe every entry.
    async fn clear(&self, #[zbus(signal_context)] ctx: SignalContext<'_>) -> u32 {
        let cleared = self.history.clear().unwrap_or(0);
        if cleared > 0 {
            if let Err(e) = Self::history_changed(&ctx).await {
                tracing::warn!("HistoryChanged emit failed: {e}");
            }
        }
        cleared
    }

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

    let db_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".jarvis/notifications.db");

    tracing::info!(db = %db_path.display(), "Starting Jarvis Notifications");

    let store = Arc::new(HistoryStore::open(&db_path).context("open notifications db")?);
    // Seed next_id from whatever's on disk so we don't reuse ids
    // across a daemon restart. New notifications continue from
    // max_id + 1.
    let starting_id = store.max_id().unwrap_or(0);
    tracing::info!(starting_id, "Seeded next_id from store");

    let service = Service {
        next_id: AsyncMutex::new(starting_id),
        history: store.clone(),
    };
    let history_iface = History {
        history: store.clone(),
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
}
