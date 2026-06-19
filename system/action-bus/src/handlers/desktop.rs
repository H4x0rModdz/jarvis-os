use crate::error::BusError;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Set the desktop wallpaper. `source` is one of:
///   - an http(s) URL  → downloaded,
///   - a local image path (`~` expands) → copied,
///   - anything else → treated as a search query and the top Wallhaven
///     result is downloaded ("troca o wallpaper por um de gato preto").
///
/// The image is written to a stable per-user path the labwc autostart also
/// reads on login (so the choice survives a reboot), then swaybg is restarted
/// to apply it live.
pub async fn set_wallpaper(params: Value) -> Result<Value, BusError> {
    let source = params["source"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'source'".into(),
        })?
        .trim();
    if source.is_empty() {
        return Err(BusError::InvalidParams {
            message: "'source' is empty".into(),
        });
    }

    let dest = wallpaper_path();
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| BusError::ExecutionFailed {
                message: format!("create {}: {e}", parent.display()),
            })?;
    }

    let mut via = "url";
    if source.contains("://") {
        download_to(source, &dest).await?;
    } else if is_local_path(source) {
        via = "file";
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
    } else {
        via = "search";
        let url = wallhaven_top(source).await?;
        download_to(&url, &dest).await?;
    }

    // Apply live: drop the running swaybg and start a fresh one on the new
    // image. swaybg inherits WAYLAND_DISPLAY from the daemon's environment.
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

    Ok(json!({ "set": true, "via": via, "path": dest.to_string_lossy() }))
}

/// Download `url` to `dest` atomically (sidecar + rename).
async fn download_to(url: &str, dest: &Path) -> Result<(), BusError> {
    let bytes = super::web::http_client()?
        .get(url)
        .send()
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("download {url}: {e}"),
        })?
        .error_for_status()
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("download {url}: {e}"),
        })?
        .bytes()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("read body: {e}"),
        })?;
    let tmp = dest.with_extension("part");
    tokio::fs::write(&tmp, &bytes)
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("write {}: {e}", tmp.display()),
        })?;
    tokio::fs::rename(&tmp, dest)
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("rename wallpaper: {e}"),
        })?;
    Ok(())
}

/// Top Wallhaven result image URL for a query. JSON API, no key (SFW only).
async fn wallhaven_top(query: &str) -> Result<String, BusError> {
    let v: Value = super::web::http_client()?
        .get("https://wallhaven.cc/api/v1/search")
        .query(&[("q", query), ("sorting", "relevance"), ("order", "desc")])
        .send()
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("wallhaven: {e}"),
        })?
        .error_for_status()
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("wallhaven: {e}"),
        })?
        .json()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("wallhaven json: {e}"),
        })?;
    v["data"][0]["path"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| BusError::NotFound {
            action: format!("wallpaper for '{query}'"),
        })
}

/// A local path looks like a path (has a slash, starts with ~, or names an
/// existing file) — otherwise we treat the string as a search query.
fn is_local_path(s: &str) -> bool {
    s.contains('/') || s.starts_with('~') || expand_tilde(s).exists()
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
