use crate::error::BusError;
use serde_json::{json, Value};

pub async fn notify(params: Value) -> Result<Value, BusError> {
    let title = params["title"].as_str().unwrap_or("Jarvis");
    let body = params["body"].as_str().unwrap_or("");
    let urgency = params["urgency"].as_str().unwrap_or("normal");
    let icon = params["icon"].as_str().unwrap_or("dialog-information");

    let output = tokio::process::Command::new("notify-send")
        .args([
            "--urgency", urgency,
            "--icon", icon,
            title,
            body,
        ])
        .output()
        .await
        .map_err(|e| BusError::ExecutionFailed { message: e.to_string() })?;

    if output.status.success() {
        Ok(json!({ "sent": true }))
    } else {
        Err(BusError::ExecutionFailed {
            message: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

pub async fn set_setting(_params: Value) -> Result<Value, BusError> {
    Err(BusError::Unavailable {
        service: "settings-daemon (not yet implemented)".into(),
    })
}

pub async fn get_setting(_params: Value) -> Result<Value, BusError> {
    Err(BusError::Unavailable {
        service: "settings-daemon (not yet implemented)".into(),
    })
}
