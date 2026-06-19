use crate::error::BusError;
use serde_json::{json, Value};

/// Type text into whatever surface currently holds keyboard focus, via
/// `wtype` (a wlroots virtual-keyboard client). The daemon inherits
/// `WAYLAND_DISPLAY` from the session (labwc's autostart imports it), so wtype
/// reaches the compositor.
///
/// Focus is the caller's responsibility — when Lilith chains "abre o zed e
/// escreve oi", she runs `app.open`/`window.focus` first, then `input.type`,
/// so the text lands in the right window.
pub async fn type_text(params: Value) -> Result<Value, BusError> {
    let text = params["text"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'text'".into(),
        })?;
    if text.is_empty() {
        return Ok(json!({ "typed": true, "len": 0 }));
    }

    let status = tokio::process::Command::new("wtype")
        .arg(text)
        .status()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("spawn wtype (is it installed?): {e}"),
        })?;

    if !status.success() {
        return Err(BusError::ExecutionFailed {
            message: format!("wtype exited with {status}"),
        });
    }
    Ok(json!({ "typed": true, "len": text.chars().count() }))
}
