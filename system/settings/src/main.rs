//! Jarvis Settings — the OS preferences daemon.
//!
//! Exposes `com.jarvis.Settings` on the session bus. Values are JSON
//! strings; the daemon validates they parse but does not interpret
//! them. See `system/settings/module.md` for the contract and ADR
//! 0008 for the rationale.

mod store;

use anyhow::Context;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use store::SettingsStore;
use zbus::{connection, interface, SignalContext};

struct SettingsService {
    store: Arc<SettingsStore>,
}

#[interface(name = "com.jarvis.Settings")]
impl SettingsService {
    /// Look up `key`. Returns JSON `{ found, key, value? }`.
    /// `value` is the stored JSON document parsed back out so callers
    /// don't have to double-decode.
    async fn get(&self, key: &str) -> String {
        match self.store.get(key) {
            Ok(Some(entry)) => {
                let value: serde_json::Value =
                    serde_json::from_str(&entry.value_json).unwrap_or(serde_json::Value::Null);
                json!({
                    "found": true,
                    "key": entry.key,
                    "value": value,
                    "updated_at": entry.updated_at,
                })
                .to_string()
            }
            Ok(None) => json!({ "found": false, "key": key }).to_string(),
            Err(e) => {
                tracing::warn!(error = %e, key, "Settings.Get failed");
                json!({ "found": false, "key": key, "error": e.to_string() }).to_string()
            }
        }
    }

    /// Store `value_json` under `key`. Returns `{ ok, error? }`.
    async fn set(
        &self,
        key: &str,
        value_json: &str,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> String {
        if let Err(e) = serde_json::from_str::<serde_json::Value>(value_json) {
            return json!({ "ok": false, "error": format!("invalid JSON: {e}") }).to_string();
        }

        match self.store.set(key, value_json) {
            Ok(()) => {
                if let Err(e) = Self::changed(&ctx, key, value_json).await {
                    tracing::warn!(error = %e, "Failed to emit Changed signal");
                }
                tracing::info!(key, "Settings.Set");
                json!({ "ok": true }).to_string()
            }
            Err(e) => {
                tracing::warn!(error = %e, key, "Settings.Set failed");
                json!({ "ok": false, "error": e.to_string() }).to_string()
            }
        }
    }

    /// Delete `key`. Returns `{ deleted }`.
    async fn delete(
        &self,
        key: &str,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> String {
        match self.store.delete(key) {
            Ok(was_present) => {
                if was_present {
                    if let Err(e) = Self::changed(&ctx, key, "").await {
                        tracing::warn!(error = %e, "Failed to emit Changed signal");
                    }
                    tracing::info!(key, "Settings.Delete");
                }
                json!({ "deleted": was_present }).to_string()
            }
            Err(e) => {
                tracing::warn!(error = %e, key, "Settings.Delete failed");
                json!({ "deleted": false, "error": e.to_string() }).to_string()
            }
        }
    }

    /// Returns `{ keys: [{ key, updated_at }] }`. Values are intentionally
    /// not included — callers `Get` what they need.
    async fn list(&self) -> String {
        match self.store.list() {
            Ok(entries) => {
                let keys: Vec<_> = entries
                    .into_iter()
                    .map(|e| json!({ "key": e.key, "updated_at": e.updated_at }))
                    .collect();
                json!({ "keys": keys }).to_string()
            }
            Err(e) => {
                tracing::warn!(error = %e, "Settings.List failed");
                json!({ "keys": [], "error": e.to_string() }).to_string()
            }
        }
    }

    /// Fires after every successful Set or Delete. Delete emits with an
    /// empty `value_json` so subscribers know the difference between
    /// "changed to null" and "removed".
    #[zbus(signal)]
    async fn changed(
        ctx: &SignalContext<'_>,
        key: &str,
        value_json: &str,
    ) -> zbus::Result<()>;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jarvis_settings=info".parse()?),
        )
        .init();

    let db_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".jarvis/settings.db");

    tracing::info!(db = %db_path.display(), "Starting Jarvis Settings");

    let store = Arc::new(SettingsStore::open(&db_path).context("open settings store")?);

    let service = SettingsService { store };

    let _conn = connection::Builder::session()?
        .name("com.jarvis.Settings")?
        .serve_at("/com/jarvis/Settings", service)?
        .build()
        .await?;

    tracing::info!("Settings ready on com.jarvis.Settings");

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}
