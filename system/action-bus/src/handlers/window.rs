use crate::error::BusError;
use serde_json::{json, Value};
use zbus::{Connection, Proxy};

// Window control is implemented on labwc via the shell, which owns the
// wlr-foreign-toplevel client (ADR 0025). We forward the verb + target
// selector to it over DBus rather than opening a second Wayland client
// here. `target` is a string: "active"/"focused" for the focused window,
// otherwise an app name (matched against app_id) or a title substring.
const SHELL_SERVICE: &str = "com.jarvis.Shell";
const SHELL_PATH: &str = "/com/jarvis/Shell";
const SHELL_IFACE: &str = "com.jarvis.Shell.Windows";

pub async fn focus(params: Value) -> Result<Value, BusError> {
    shell_window_call("Focus", &target_of(&params)).await
}

pub async fn minimize(params: Value) -> Result<Value, BusError> {
    shell_window_call("Minimize", &target_of(&params)).await
}

pub async fn maximize(params: Value) -> Result<Value, BusError> {
    shell_window_call("Maximize", &target_of(&params)).await
}

pub async fn close(params: Value) -> Result<Value, BusError> {
    shell_window_call("Close", &target_of(&params)).await
}

// Geometry + snapping are not in wlr-foreign-toplevel and labwc exposes no
// IPC for them; they need the Jarvis (Smithay) compositor (ADR 0024/0025).
pub async fn move_window(_params: Value) -> Result<Value, BusError> {
    compositor_deferred("window.move")
}

pub async fn resize(_params: Value) -> Result<Value, BusError> {
    compositor_deferred("window.resize")
}

pub async fn snap_left(_params: Value) -> Result<Value, BusError> {
    compositor_deferred("window.snap_left")
}

pub async fn snap_right(_params: Value) -> Result<Value, BusError> {
    compositor_deferred("window.snap_right")
}

/// Pick the target selector from params, defaulting to the focused window.
/// An empty string is treated as absent so callers can't accidentally
/// target "" (which matches nothing).
fn target_of(params: &Value) -> String {
    params["target"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("active")
        .to_string()
}

async fn shell_window_call(method: &str, target: &str) -> Result<Value, BusError> {
    let proxy = shell_proxy().await?;
    let matched: bool =
        proxy
            .call(method, &(target,))
            .await
            .map_err(|e| BusError::Unavailable {
                service: format!("Shell.{method}: {e}"),
            })?;
    if matched {
        Ok(json!({ "ok": true, "target": target }))
    } else {
        Err(BusError::ExecutionFailed {
            message: format!("no window matched target '{target}'"),
        })
    }
}

async fn shell_proxy() -> Result<Proxy<'static>, BusError> {
    let conn = Connection::session()
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("session bus: {e}"),
        })?;
    Proxy::new(&conn, SHELL_SERVICE, SHELL_PATH, SHELL_IFACE)
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Shell proxy: {e}"),
        })
}

fn compositor_deferred(action: &str) -> Result<Value, BusError> {
    Err(BusError::Unavailable {
        service: format!("{action}: needs geometry control from the Jarvis compositor (deferred)"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_defaults_to_active() {
        assert_eq!(target_of(&json!({})), "active");
        assert_eq!(target_of(&json!({ "target": "" })), "active");
        assert_eq!(target_of(&json!({ "target": "  " })), "active");
    }

    #[test]
    fn target_passes_through_app_name() {
        assert_eq!(target_of(&json!({ "target": "firefox" })), "firefox");
        assert_eq!(target_of(&json!({ "target": " Zed " })), "Zed");
    }

    #[tokio::test]
    async fn geometry_actions_are_deferred() {
        for r in [
            move_window(json!({})).await,
            resize(json!({})).await,
            snap_left(json!({})).await,
            snap_right(json!({})).await,
        ] {
            assert!(matches!(r, Err(BusError::Unavailable { .. })));
        }
    }
}
