use crate::error::BusError;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Write a string to the system clipboard.
///
/// Tries `wl-copy` first (the Wayland clipboard tool that labwc/wlroots
/// expect). Falls back to `xclip` so the same handler works under an X11
/// session — useful for development on Ubuntu / WSLg where Wayland may not
/// be running.
///
/// MIME type defaults to `text/plain` but can be overridden so callers can
/// drop rich text or other formats onto the clipboard.
pub async fn set(params: Value) -> Result<Value, BusError> {
    let content = params["text"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'text'".into(),
        })?;
    let mime = params["mime"].as_str().unwrap_or("text/plain");

    let (cmd, args): (&str, Vec<String>) = if super::which_exists("wl-copy").await {
        ("wl-copy", vec!["--type".into(), mime.into()])
    } else if super::which_exists("xclip").await {
        (
            "xclip",
            vec![
                "-selection".into(),
                "clipboard".into(),
                "-t".into(),
                mime.into(),
            ],
        )
    } else {
        return Err(BusError::Unavailable {
            service: "no clipboard tool available (need wl-clipboard or xclip)".into(),
        });
    };

    let mut child = Command::new(cmd)
        .args(&args)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("{cmd}: {e}"),
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(content.as_bytes())
            .await
            .map_err(|e| BusError::ExecutionFailed {
                message: format!("write to {cmd} stdin: {e}"),
            })?;
        // Drop stdin so the child sees EOF and persists the contents.
    }

    let status = child.wait().await.map_err(|e| BusError::ExecutionFailed {
        message: format!("wait on {cmd}: {e}"),
    })?;
    if !status.success() {
        return Err(BusError::ExecutionFailed {
            message: format!("{cmd} exited with {status}"),
        });
    }

    Ok(json!({ "written": true, "bytes": content.len(), "mime": mime, "tool": cmd }))
}

/// Read the current clipboard contents.
///
/// Same `wl-paste` → `xclip` fallback as `set`. Returns an empty string when
/// the clipboard is empty (matches the underlying tools' behavior).
pub async fn get(_params: Value) -> Result<Value, BusError> {
    let (cmd, args): (&str, Vec<&str>) = if super::which_exists("wl-paste").await {
        ("wl-paste", vec!["--no-newline"])
    } else if super::which_exists("xclip").await {
        ("xclip", vec!["-selection", "clipboard", "-o"])
    } else {
        return Err(BusError::Unavailable {
            service: "no clipboard tool available (need wl-clipboard or xclip)".into(),
        });
    };

    let output =
        Command::new(cmd)
            .args(&args)
            .output()
            .await
            .map_err(|e| BusError::ExecutionFailed {
                message: format!("{cmd}: {e}"),
            })?;

    // `wl-paste` exits 1 when the clipboard is empty. Treat that as the
    // empty-string result the caller almost certainly wants — Lilith would
    // otherwise have to special-case the error.
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("empty") || stderr.contains("No selection") {
            return Ok(json!({ "text": "", "tool": cmd }));
        }
        return Err(BusError::ExecutionFailed {
            message: format!("{cmd} failed: {}", stderr.trim()),
        });
    }

    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(json!({ "text": text, "tool": cmd }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_requires_text_param() {
        let r = set(json!({})).await;
        assert!(matches!(r, Err(BusError::InvalidParams { .. })));
    }

    #[tokio::test]
    async fn which_detects_a_universal_binary() {
        // `sh` is present on every Linux + CI runner; this confirms the
        // shared probe actually walks PATH instead of always returning false.
        if cfg!(target_os = "linux") {
            assert!(super::super::which_exists("sh").await);
        }
    }

    #[tokio::test]
    async fn which_returns_false_for_nonsense() {
        assert!(!super::super::which_exists("definitely-not-a-real-binary-xyz").await);
    }
}
