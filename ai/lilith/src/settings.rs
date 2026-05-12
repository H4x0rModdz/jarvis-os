//! Thin async helper for reading a single setting from
//! `com.jarvis.Settings`. The Settings daemon is `Requires=` in the
//! session target, so it should always be up by the time Lilith runs —
//! but we still degrade gracefully (return `None`) when the bus call
//! errors out so a missing daemon never blocks Lilith from starting.

use serde_json::Value;
use zbus::{Connection, Proxy};

const SETTINGS_SERVICE: &str = "com.jarvis.Settings";
const SETTINGS_PATH: &str = "/com/jarvis/Settings";
const SETTINGS_IFACE: &str = "com.jarvis.Settings";

/// Reads `key` from Settings. Returns the inner JSON `value` field as a
/// string when `found: true`, `None` otherwise (including all bus-error
/// paths). Callers chain this through a default the same way they would
/// `std::env::var`.
pub async fn read_string(key: &str) -> Option<String> {
    let conn = Connection::session().await.ok()?;
    let proxy = Proxy::new(&conn, SETTINGS_SERVICE, SETTINGS_PATH, SETTINGS_IFACE)
        .await
        .ok()?;
    let response: String = proxy.call("Get", &(key,)).await.ok()?;
    let parsed: Value = serde_json::from_str(&response).ok()?;
    if !parsed.get("found")?.as_bool()? {
        return None;
    }
    parsed.get("value")?.as_str().map(|s| s.to_string())
}
