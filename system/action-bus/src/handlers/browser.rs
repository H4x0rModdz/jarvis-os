use crate::error::BusError;
use serde_json::{json, Value};

/// Open `url` in the user's default browser via `xdg-open`.
///
/// `xdg-open` does the heavy lifting of resolving the default browser from
/// the desktop database. On bootc systems with no GUI browser installed
/// the call will still succeed at the dispatch level — the handler reports
/// `launched: true` as soon as xdg-open has been spawned — but the user
/// will see no window. Action Bus does not currently model "process
/// outlived the call but produced no visible result"; that's a Phase 3
/// observability problem.
pub async fn open(params: Value) -> Result<Value, BusError> {
    let url = params["url"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'url'".into(),
        })?;

    if !is_safe_url(url) {
        return Err(BusError::InvalidParams {
            message: format!("refusing to open url with disallowed scheme: {url}"),
        });
    }

    let child = tokio::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("xdg-open failed: {e}"),
        })?;

    Ok(json!({ "launched": true, "pid": child.id(), "url": url }))
}

/// Cheap allowlist on URL schemes — we only want to spawn xdg-open for
/// things a browser is expected to handle. Blocks `file://` (would leak
/// local-FS info), `javascript:` (script injection) and any random custom
/// scheme that a malicious caller could use to trigger a registered handler
/// in unexpected ways.
fn is_safe_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_and_https_pass() {
        assert!(is_safe_url("http://example.com"));
        assert!(is_safe_url("https://example.com/path?q=1"));
        assert!(is_safe_url("HTTPS://EXAMPLE.COM"));
    }

    #[test]
    fn mailto_passes() {
        assert!(is_safe_url("mailto:test@example.com"));
    }

    #[test]
    fn file_url_rejected() {
        assert!(!is_safe_url("file:///etc/passwd"));
    }

    #[test]
    fn javascript_url_rejected() {
        assert!(!is_safe_url("javascript:alert(1)"));
    }

    #[test]
    fn custom_scheme_rejected() {
        assert!(!is_safe_url("steam://run/440"));
    }

    #[tokio::test]
    async fn missing_url_returns_invalid_params() {
        let r = open(json!({})).await;
        assert!(matches!(r, Err(BusError::InvalidParams { .. })));
    }

    #[tokio::test]
    async fn disallowed_scheme_returns_invalid_params() {
        let r = open(json!({ "url": "file:///etc/passwd" })).await;
        assert!(matches!(r, Err(BusError::InvalidParams { .. })));
    }
}
