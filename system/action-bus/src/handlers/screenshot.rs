use crate::error::BusError;
use chrono::Local;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::process::Command;

/// Capture a screenshot and write it to disk.
///
/// Wayland path uses `grim` (the standard wlroots screenshot tool). The
/// X11 fallback is `scrot`. Region selection (`mode: "region"`) requires
/// `slurp` for Wayland.
///
/// Params:
/// - `path` (optional): absolute filename. Default
///   `~/Pictures/Screenshots/Jarvis-<timestamp>.png`.
/// - `mode` (optional): "full" | "region". Default "full".
pub async fn capture(params: Value) -> Result<Value, BusError> {
    let mode = params["mode"].as_str().unwrap_or("full");
    let target_path = resolve_target_path(&params)?;

    // Ensure parent dir exists. This is the rare case where it's worth
    // creating directories for the caller — the user shouldn't have to
    // mkdir Screenshots/ on first invocation.
    if let Some(parent) = target_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| BusError::ExecutionFailed {
                message: format!("mkdir {}: {e}", parent.display()),
            })?;
    }

    let path_str = target_path.to_string_lossy().into_owned();

    let cmd = capture_command(mode, &path_str).await?;
    let (tool, args) = cmd;

    let output = Command::new(tool)
        .args(args.iter().map(|s| s.as_str()))
        .output()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("{tool}: {e}"),
        })?;

    if !output.status.success() {
        return Err(BusError::ExecutionFailed {
            message: format!(
                "{tool} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }

    Ok(json!({
        "saved": true,
        "path": path_str,
        "mode": mode,
        "tool": tool,
    }))
}

fn resolve_target_path(params: &Value) -> Result<PathBuf, BusError> {
    if let Some(p) = params["path"].as_str() {
        return Ok(PathBuf::from(p));
    }

    let home = dirs::home_dir().ok_or_else(|| BusError::ExecutionFailed {
        message: "no HOME directory; pass `path` explicitly".into(),
    })?;

    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    Ok(home
        .join("Pictures")
        .join("Screenshots")
        .join(format!("Jarvis-{stamp}.png")))
}

async fn capture_command(
    mode: &str,
    path: &str,
) -> Result<(&'static str, Vec<String>), BusError> {
    let wayland = std::env::var("WAYLAND_DISPLAY").is_ok();

    match (wayland, mode) {
        (true, "region") => {
            if !super::which_exists("slurp").await {
                return Err(BusError::Unavailable {
                    service: "slurp (region selection on Wayland)".into(),
                });
            }
            if !super::which_exists("grim").await {
                return Err(BusError::Unavailable {
                    service: "grim (Wayland screenshot tool)".into(),
                });
            }
            // grim reads the region from stdin via -g.
            // We invoke a shell to pipe slurp into grim — keeps us off any
            // extra dependency just for command composition.
            Ok((
                "sh",
                vec![
                    "-c".into(),
                    format!("grim -g \"$(slurp)\" \"{path}\""),
                ],
            ))
        }
        (true, _) => {
            if super::which_exists("grim").await {
                Ok(("grim", vec![path.into()]))
            } else {
                Err(BusError::Unavailable {
                    service: "grim (Wayland screenshot tool)".into(),
                })
            }
        }
        (false, "region") => Ok(("scrot", vec!["--select".into(), path.into()])),
        (false, _) => Ok(("scrot", vec![path.into()])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_lives_under_pictures_screenshots() {
        let p = resolve_target_path(&json!({})).expect("home dir present in tests");
        let s = p.to_string_lossy();
        assert!(
            s.contains("Pictures") && s.contains("Screenshots"),
            "{s}"
        );
        assert!(s.ends_with(".png"), "{s}");
    }

    #[test]
    fn explicit_path_is_honored() {
        let p = resolve_target_path(&json!({ "path": "/tmp/foo.png" })).unwrap();
        assert_eq!(p.to_string_lossy(), "/tmp/foo.png");
    }
}
