use crate::error::BusError;
use serde_json::{json, Value};
use zbus::{Connection, Proxy};

const SETTINGS_SERVICE: &str = "com.jarvis.Settings";
const SETTINGS_PATH: &str = "/com/jarvis/Settings";
const SETTINGS_IFACE: &str = "com.jarvis.Settings";

pub async fn notify(params: Value) -> Result<Value, BusError> {
    let title = params["title"].as_str().unwrap_or("Jarvis");
    let body = params["body"].as_str().unwrap_or("");
    let urgency = params["urgency"].as_str().unwrap_or("normal");
    let icon = params["icon"].as_str().unwrap_or("dialog-information");

    let output = tokio::process::Command::new("notify-send")
        .args(["--urgency", urgency, "--icon", icon, title, body])
        .output()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: e.to_string(),
        })?;

    if output.status.success() {
        Ok(json!({ "sent": true }))
    } else {
        Err(BusError::ExecutionFailed {
            message: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// Read a setting from the Settings daemon.
///
/// Accepts `key` and an optional `default` (any JSON value). When the
/// daemon answers `found: false`, `default` is returned if provided, else
/// `null`. Lilith can call this without first checking existence —
/// `default` covers the common "what's the user's preferred X, fall back
/// to Y" pattern in one round trip.
pub async fn get_setting(params: Value) -> Result<Value, BusError> {
    let key = params["key"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'key'".into(),
        })?;
    let default = params.get("default").cloned();

    let proxy = settings_proxy().await?;
    let response: String = proxy
        .call("Get", &(key,))
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Settings.Get: {e}"),
        })?;

    let parsed: Value = serde_json::from_str(&response).map_err(|e| BusError::ExecutionFailed {
        message: format!("Settings returned non-JSON: {e}"),
    })?;

    let found = parsed["found"].as_bool().unwrap_or(false);
    let value = parsed.get("value").cloned();

    Ok(json!({
        "found": found,
        "key": key,
        "value": value.or(default).unwrap_or(Value::Null),
        "updated_at": parsed.get("updated_at").cloned().unwrap_or(Value::Null),
    }))
}

/// Write a setting to the Settings daemon.
///
/// `value` is any JSON value. We serialize it back to a JSON string for
/// the wire — the Settings daemon stores the bytes verbatim and validates
/// they parse on the other side.
pub async fn set_setting(params: Value) -> Result<Value, BusError> {
    let key = params["key"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'key'".into(),
        })?;
    let value = params.get("value").ok_or_else(|| BusError::InvalidParams {
        message: "missing required param 'value'".into(),
    })?;
    let value_json = serde_json::to_string(value).map_err(|e| BusError::InvalidParams {
        message: format!("could not serialise 'value' to JSON: {e}"),
    })?;

    let proxy = settings_proxy().await?;
    let response: String = proxy
        .call("Set", &(key, value_json.as_str()))
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Settings.Set: {e}"),
        })?;

    let parsed: Value = serde_json::from_str(&response).map_err(|e| BusError::ExecutionFailed {
        message: format!("Settings returned non-JSON: {e}"),
    })?;

    if parsed["ok"].as_bool() == Some(true) {
        Ok(json!({ "set": true, "key": key }))
    } else {
        let msg = parsed["error"].as_str().unwrap_or("unknown error").to_string();
        Err(BusError::ExecutionFailed { message: msg })
    }
}

async fn settings_proxy() -> Result<Proxy<'static>, BusError> {
    let conn = Connection::session()
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("session bus: {e}"),
        })?;
    Proxy::new(&conn, SETTINGS_SERVICE, SETTINGS_PATH, SETTINGS_IFACE)
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Settings proxy: {e}"),
        })
}
