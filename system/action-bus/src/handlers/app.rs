use crate::error::BusError;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Open an app, a folder, or a URL.
///
/// The old version just ran `xdg-open <arg>`. That works for paths
/// and URLs but SILENTLY no-ops for a bare app name like "firefox":
/// xdg-open spawns fine (the binary exists) so the fallback never
/// fires, but xdg-open does nothing with a non-path/non-URL word —
/// and the pre-installed apps are Flatpaks (org.mozilla.firefox,
/// org.kde.dolphin, dev.zed.Zed) with no plain `firefox` binary in
/// PATH. Result: Lilith reported success while nothing opened.
///
/// New resolution order:
///   1. Path / URL / existing file or dir → `xdg-open` (the MIME
///      defaults route dirs to Dolphin, https to Firefox, etc.).
///   2. App name → find the matching .desktop and `gio launch` it.
///      gio honours a Flatpak desktop's `Exec=flatpak run …`, which
///      a bare exec can't. "firefox" matches org.mozilla.firefox by
///      a case-insensitive id match.
///   3. Last resort → exec the name directly (real binaries: foot…).
pub async fn open(params: Value) -> Result<Value, BusError> {
    let app = require_str(&params, "app")?;

    // 1. Paths, URLs, existing files/dirs → xdg-open.
    let is_target = app.contains("://")
        || app.starts_with('/')
        || app.starts_with('~')
        || Path::new(app).exists();
    if is_target {
        // Prefer `gio open` over `xdg-open`. On the Jarvis session
        // `xdg-open`'s desktop detection doesn't recognise our DE, so it
        // falls through to a text-browser chain (www-browser, lynx, …)
        // and fails to open folders/URLs — it never consults the MIME
        // defaults. `gio open` reads the same mimeapps.list defaults
        // directly (folders → Dolphin, http → Firefox) and routes
        // correctly. xdg-open stays as the fallback for hosts without gio.
        if let Ok(c) = session_command("gio").arg("open").arg(app).spawn() {
            return Ok(json!({ "launched": true, "pid": c.id(), "via": "gio-open" }));
        }
        let child = session_command("xdg-open").arg(app).spawn().map_err(|e| {
            BusError::ExecutionFailed {
                message: format!("xdg-open '{app}': {e}"),
            }
        })?;
        return Ok(json!({ "launched": true, "pid": child.id(), "via": "xdg-open" }));
    }

    // 2. App name → resolve a .desktop, launch via gio (Flatpak-aware).
    if let Some(desktop) = resolve_desktop(app) {
        if let Ok(c) = session_command("gio").arg("launch").arg(&desktop).spawn() {
            return Ok(json!({
                "launched": true,
                "pid": c.id(),
                "via": "gio",
                "desktop": desktop.to_string_lossy(),
            }));
        }
        // gio absent → try gtk-launch by desktop id.
        if let Some(id) = desktop.file_stem().and_then(|s| s.to_str()) {
            if let Ok(c) = session_command("gtk-launch").arg(id).spawn() {
                return Ok(json!({ "launched": true, "pid": c.id(), "via": "gtk-launch" }));
            }
        }
    }

    // 3. Last resort: a real binary on PATH.
    let child = session_command(app)
        .spawn()
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("Failed to launch '{app}': {e}"),
        })?;
    Ok(json!({ "launched": true, "pid": child.id(), "via": "exec" }))
}

/// Build a launch command pre-seeded with the graphical-session environment
/// that GUI apps need.
///
/// jarvis-action-bus is a systemd *user* service that usually starts before
/// labwc imports `WAYLAND_DISPLAY` into the user manager, so the daemon's own
/// environment lacks it. An app launched without `WAYLAND_DISPLAY` falls back
/// to X11, finds no `DISPLAY`, and dies on a headless Wayland session —
/// exactly why Firefox did nothing from the dock yet opened fine from a shell
/// that had the var exported. The child still inherits everything else
/// (incl. `XDG_RUNTIME_DIR`, which systemd does set); we only fill the gap.
fn session_command(program: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        if let Some(display) = detect_wayland_display() {
            cmd.env("WAYLAND_DISPLAY", display);
        }
    }
    cmd
}

/// Find the Wayland socket name under `XDG_RUNTIME_DIR` — `wayland-0` in the
/// common case, else the first `wayland-N` present (ignoring the `.lock`).
fn detect_wayland_display() -> Option<String> {
    let rt = std::env::var_os("XDG_RUNTIME_DIR")?;
    let dir = std::path::Path::new(&rt);
    if dir.join("wayland-0").exists() {
        return Some("wayland-0".to_string());
    }
    std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
        let name = e.file_name().into_string().ok()?;
        (name.starts_with("wayland-") && !name.ends_with(".lock")).then_some(name)
    })
}

/// Find the best `.desktop` for an app name across the standard XDG
/// and Flatpak export dirs. Match priority: exact basename, then the
/// last dot-segment (org.mozilla.firefox to "firefox"), then any
/// case-insensitive substring. Returns the file path for `gio launch`.
fn resolve_desktop(app: &str) -> Option<PathBuf> {
    let needle = app.to_lowercase();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(format!(
            "{home}/.local/share/flatpak/exports/share/applications"
        )));
        dirs.push(PathBuf::from(format!("{home}/.local/share/applications")));
    }
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs.push(PathBuf::from("/usr/local/share/applications"));

    let mut last_seg: Option<PathBuf> = None;
    let mut substring: Option<PathBuf> = None;
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("desktop") {
                continue;
            }
            let stem = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            if stem == needle {
                return Some(p); // exact id wins outright
            }
            if last_seg.is_none() && stem.rsplit('.').next() == Some(needle.as_str()) {
                last_seg = Some(p.clone());
            }
            if substring.is_none() && stem.contains(&needle) {
                substring = Some(p);
            }
        }
    }
    last_seg.or(substring)
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
