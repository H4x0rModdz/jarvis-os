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

/// Install a Linux application via Flatpak.
///
/// V1 routes everything through Flathub. The caller passes a Flatpak
/// app id (`org.mozilla.firefox`), not a human-readable name —
/// resolving "firefox" to its app id is Lilith's job once she has a
/// fuzzy-match index. The install is `--user` (per-user, no root
/// needed) and non-interactive.
///
/// Windows binaries go through `compat.run_exe` instead (Phase 3
/// compat layer). This action is the Linux-native path.
pub async fn install(params: Value) -> Result<Value, BusError> {
    let app_id = require_str(&params, "app_id")?;

    if !super::which_exists("flatpak").await {
        return Err(BusError::Unavailable {
            service: "flatpak not installed".into(),
        });
    }

    let output = tokio::process::Command::new("flatpak")
        .args([
            "install",
            "--user",
            "--noninteractive",
            "--assumeyes",
            "flathub",
            app_id,
        ])
        .output()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("spawn flatpak: {e}"),
        })?;

    if output.status.success() {
        Ok(json!({ "installed": true, "app_id": app_id, "source": "flathub" }))
    } else {
        Err(BusError::ExecutionFailed {
            message: format!(
                "flatpak install {app_id}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        })
    }
}

/// Uninstall a Flatpak-installed app.
pub async fn uninstall(params: Value) -> Result<Value, BusError> {
    let app_id = require_str(&params, "app_id")?;

    if !super::which_exists("flatpak").await {
        return Err(BusError::Unavailable {
            service: "flatpak not installed".into(),
        });
    }

    let output = tokio::process::Command::new("flatpak")
        .args([
            "uninstall",
            "--user",
            "--noninteractive",
            "--assumeyes",
            app_id,
        ])
        .output()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("spawn flatpak: {e}"),
        })?;

    if output.status.success() {
        Ok(json!({ "uninstalled": true, "app_id": app_id }))
    } else {
        Err(BusError::ExecutionFailed {
            message: format!(
                "flatpak uninstall {app_id}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        })
    }
}

fn require_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, BusError> {
    params[key].as_str().ok_or_else(|| BusError::InvalidParams {
        message: format!("missing required param '{key}'"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn install_requires_app_id() {
        let r = install(json!({})).await;
        assert!(matches!(r, Err(BusError::InvalidParams { .. })));
    }

    #[tokio::test]
    async fn uninstall_requires_app_id() {
        let r = uninstall(json!({})).await;
        assert!(matches!(r, Err(BusError::InvalidParams { .. })));
    }
}
