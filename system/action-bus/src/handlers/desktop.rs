use crate::error::BusError;
use serde_json::{json, Value};
use std::path::PathBuf;

/// Set the desktop wallpaper from a local path or a URL.
///
/// Writes the chosen image to a stable per-user path that the labwc autostart
/// also reads on login, so the choice survives a reboot, then restarts swaybg
/// to apply it live. URLs are fetched with curl (shipped in the image).
pub async fn set_wallpaper(params: Value) -> Result<Value, BusError> {
    let source = params["source"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'source'".into(),
        })?;

    let dest = wallpaper_path();
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| BusError::ExecutionFailed {
                message: format!("create {}: {e}", parent.display()),
            })?;
    }

    if source.contains("://") {
        // Remote: download to a sidecar then rename, so a failed/partial
        // fetch never replaces the current wallpaper.
        let tmp = dest.with_extension("part");
        let status = tokio::process::Command::new("curl")
            .arg("-fL")
            .arg("--retry")
            .arg("3")
            .arg("-o")
            .arg(&tmp)
            .arg(source)
            .status()
            .await
            .map_err(|e| BusError::ExecutionFailed {
                message: format!("spawn curl: {e}"),
            })?;
        if !status.success() {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(BusError::ExecutionFailed {
                message: format!("download failed ({status}) for {source}"),
            });
        }
        tokio::fs::rename(&tmp, &dest)
            .await
            .map_err(|e| BusError::ExecutionFailed {
                message: format!("rename wallpaper: {e}"),
            })?;
    } else {
        // Local file.
        let src = expand_tilde(source);
        if !src.exists() {
            return Err(BusError::NotFound {
                action: format!("file '{}'", src.display()),
            });
        }
        tokio::fs::copy(&src, &dest)
            .await
            .map_err(|e| BusError::ExecutionFailed {
                message: format!("copy {}: {e}", src.display()),
            })?;
    }

    // Apply live: drop the running swaybg and start a fresh one on the new
    // image. On the next login the autostart reads the same path, so the
    // choice persists. swaybg inherits WAYLAND_DISPLAY from the daemon.
    let _ = tokio::process::Command::new("pkill")
        .args(["-x", "swaybg"])
        .status()
        .await;
    tokio::process::Command::new("swaybg")
        .arg("-m")
        .arg("fill")
        .arg("-i")
        .arg(&dest)
        .spawn()
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("spawn swaybg: {e}"),
        })?;

    Ok(json!({ "set": true, "path": dest.to_string_lossy() }))
}

/// Stable per-user wallpaper file — `~/.local/share/jarvis/wallpaper`. The
/// labwc autostart reads this exact path so a wallpaper set here comes back
/// after a reboot.
fn wallpaper_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("jarvis")
        .join("wallpaper")
}

fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(p)
}
