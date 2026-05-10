use crate::error::BusError;
use serde_json::{json, Value};

pub async fn open(params: Value) -> Result<Value, BusError> {
    let app = require_str(&params, "app")?;

    // Try xdg-open first (handles .desktop IDs and URLs), fall back to direct exec
    let child = tokio::process::Command::new("xdg-open")
        .arg(app)
        .spawn()
        .or_else(|_| tokio::process::Command::new(app).spawn())
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("Failed to launch '{app}': {e}"),
        })?;

    Ok(json!({ "launched": true, "pid": child.id() }))
}

pub async fn close(params: Value) -> Result<Value, BusError> {
    let app = require_str(&params, "app")?;
    let force = params["force"].as_bool().unwrap_or(false);
    let signal = if force { "KILL" } else { "TERM" };

    let output = tokio::process::Command::new("pkill")
        .args([&format!("-{signal}"), app])
        .output()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: e.to_string(),
        })?;

    if output.status.success() {
        Ok(json!({ "closed": true, "signal": signal }))
    } else {
        Err(BusError::NotFound {
            action: format!("process '{app}'"),
        })
    }
}

pub async fn install(_params: Value) -> Result<Value, BusError> {
    // Stub: implemented by the compatibility layer module
    Err(BusError::Unavailable {
        service: "app-installer (not yet implemented)".into(),
    })
}

pub async fn uninstall(_params: Value) -> Result<Value, BusError> {
    Err(BusError::Unavailable {
        service: "app-uninstaller (not yet implemented)".into(),
    })
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, BusError> {
    params[key].as_str().ok_or_else(|| BusError::InvalidParams {
        message: format!("missing required param '{key}'"),
    })
}
