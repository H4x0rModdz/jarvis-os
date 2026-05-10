use crate::error::BusError;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::fs;

pub async fn move_file(params: Value) -> Result<Value, BusError> {
    let source: PathBuf = require_path(&params, "source")?;
    let destination: PathBuf = require_path(&params, "destination")?;

    fs::rename(&source, &destination)
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: e.to_string(),
        })?;

    Ok(json!({
        "moved": true,
        "source": source.to_string_lossy(),
        "destination": destination.to_string_lossy()
    }))
}

pub async fn copy_file(params: Value) -> Result<Value, BusError> {
    let source: PathBuf = require_path(&params, "source")?;
    let destination: PathBuf = require_path(&params, "destination")?;

    let bytes = fs::copy(&source, &destination)
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: e.to_string(),
        })?;

    Ok(json!({
        "copied": true,
        "bytes": bytes,
        "destination": destination.to_string_lossy()
    }))
}

pub async fn delete(params: Value) -> Result<Value, BusError> {
    let path: PathBuf = require_path(&params, "path")?;
    let permanent = params["permanent"].as_bool().unwrap_or(false);

    if permanent {
        if path.is_dir() {
            fs::remove_dir_all(&path).await
        } else {
            fs::remove_file(&path).await
        }
        .map_err(|e| BusError::ExecutionFailed {
            message: e.to_string(),
        })?;

        Ok(json!({ "deleted": true, "permanent": true }))
    } else {
        // Move to trash via gio (standard on Fedora/GNOME-based systems)
        let output = tokio::process::Command::new("gio")
            .args(["trash", path.to_str().unwrap_or("")])
            .output()
            .await
            .map_err(|e| BusError::ExecutionFailed {
                message: e.to_string(),
            })?;

        if output.status.success() {
            Ok(json!({ "deleted": true, "permanent": false }))
        } else {
            Err(BusError::ExecutionFailed {
                message: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }
}

fn require_path(params: &Value, key: &str) -> Result<PathBuf, BusError> {
    params[key]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| BusError::InvalidParams {
            message: format!("missing required param '{key}'"),
        })
}
