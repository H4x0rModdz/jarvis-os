use crate::error::BusError;
use serde_json::{json, Value};
use zbus::{Connection, Proxy};

const UPDATER_SERVICE: &str = "com.jarvis.Updater";
const UPDATER_PATH: &str = "/com/jarvis/Updater";
const UPDATER_IFACE: &str = "com.jarvis.Updater";

/// Ask the Updater daemon what's installed and whether an OS upgrade
/// is staged. The result is the raw JSON from `Updater.Check`,
/// re-parsed into the action response so Lilith / the shell don't have
/// to double-decode.
pub async fn check(_params: Value) -> Result<Value, BusError> {
    let proxy = updater_proxy().await?;
    let response: String = proxy
        .call("Check", &())
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Updater.Check: {e}"),
        })?;

    serde_json::from_str::<Value>(&response).map_err(|e| BusError::ExecutionFailed {
        message: format!("Updater returned non-JSON: {e}"),
    })
}

/// Kick off the bootc OS upgrade flow. The daemon owns the actual work;
/// callers track progress via the `Progress` / `Completed` signals on
/// `com.jarvis.Updater`. Returns the daemon's `{ started, reason? }`
/// envelope.
pub async fn apply_os(_params: Value) -> Result<Value, BusError> {
    let proxy = updater_proxy().await?;
    let response: String =
        proxy
            .call("ApplyOSUpgrade", &())
            .await
            .map_err(|e| BusError::Unavailable {
                service: format!("Updater.ApplyOSUpgrade: {e}"),
            })?;

    let parsed: Value = serde_json::from_str(&response).map_err(|e| BusError::ExecutionFailed {
        message: format!("Updater returned non-JSON: {e}"),
    })?;

    if parsed["started"].as_bool() == Some(true) {
        Ok(json!({ "started": true }))
    } else {
        let reason = parsed["reason"]
            .as_str()
            .unwrap_or("unknown reason")
            .to_string();
        Err(BusError::ExecutionFailed { message: reason })
    }
}

async fn updater_proxy() -> Result<Proxy<'static>, BusError> {
    let conn = Connection::session()
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("session bus: {e}"),
        })?;
    Proxy::new(&conn, UPDATER_SERVICE, UPDATER_PATH, UPDATER_IFACE)
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Updater proxy: {e}"),
        })
}
