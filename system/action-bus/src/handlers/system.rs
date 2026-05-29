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

    // Route through org.freedesktop.Notifications (owned by
    // jarvis-notifications). This gives third-party `notify-send`
    // calls and Lilith's notifications the same UX path, and it lets
    // Lilith query `RecentNotifications()` later to see what's been
    // surfaced.
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("session bus: {e}"),
        })?;
    let proxy = zbus::Proxy::new(
        &conn,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )
    .await
    .map_err(|e| BusError::Unavailable {
        service: format!("Notifications proxy: {e}"),
    })?;

    // FreeDesktop urgency hint: 0=low, 1=normal, 2=critical.
    let urgency_byte: u8 = match urgency {
        "low" => 0,
        "critical" => 2,
        _ => 1,
    };

    let actions: Vec<&str> = Vec::new();
    let mut hints: std::collections::HashMap<&str, zbus::zvariant::Value> =
        std::collections::HashMap::new();
    hints.insert("urgency", zbus::zvariant::Value::U8(urgency_byte));

    let id: u32 = proxy
        .call(
            "Notify",
            &(
                "Jarvis", // app_name
                0u32,     // replaces_id
                icon,     // app_icon
                title,    // summary
                body,     // body
                actions,  // actions
                hints,    // hints
                -1i32,    // expire_timeout (-1 = server default)
            ),
        )
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("Notifications.Notify: {e}"),
        })?;

    Ok(json!({ "sent": true, "id": id }))
}

/// Power management: shut down, reboot, suspend, or lock the session.
///
/// `op` is one of `poweroff` / `reboot` / `suspend` / `lock`. The first
/// three go through `systemctl` (logind grants an active local session
/// these via polkit without sudo); `lock` goes through
/// `loginctl lock-session`, which jarvis-lock picks up the same way the
/// idle auto-lock does. This is the backend for the Jarvis menu's power
/// items — the shell can't shell out itself (Action Bus boundary), so it
/// dispatches here.
///
/// The scope is `system.power`, deliberately NOT on the safe list, so a
/// stray Lilith call would prompt for confirmation. The menu items are
/// already an explicit user gesture, so the shell dispatches them
/// directly.
pub async fn power(params: Value) -> Result<Value, BusError> {
    let op = params["op"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'op' (poweroff|reboot|suspend|lock)".into(),
        })?;

    // (binary, args) per op. Unknown ops are rejected before we spawn so
    // we never hand an arbitrary verb to systemctl.
    let (bin, args): (&str, &[&str]) = match op {
        "poweroff" => ("systemctl", &["poweroff"]),
        "reboot" => ("systemctl", &["reboot"]),
        "suspend" => ("systemctl", &["suspend"]),
        "lock" => ("loginctl", &["lock-session"]),
        other => {
            return Err(BusError::InvalidParams {
                message: format!("unknown power op '{other}' (poweroff|reboot|suspend|lock)"),
            })
        }
    };

    let child = tokio::process::Command::new(bin)
        .args(args)
        .spawn()
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("spawn {bin} {}: {e}", args.join(" ")),
        })?;

    Ok(json!({ "ok": true, "op": op, "pid": child.id() }))
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
        let msg = parsed["error"]
            .as_str()
            .unwrap_or("unknown error")
            .to_string();
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

#[cfg(test)]
mod power_tests {
    use super::*;

    #[tokio::test]
    async fn power_requires_op() {
        let r = power(json!({})).await;
        assert!(matches!(r, Err(BusError::InvalidParams { .. })));
    }

    #[tokio::test]
    async fn power_rejects_unknown_op() {
        let r = power(json!({ "op": "self-destruct" })).await;
        assert!(matches!(r, Err(BusError::InvalidParams { .. })));
    }
}
