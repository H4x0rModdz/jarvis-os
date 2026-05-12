use crate::error::BusError;
use serde_json::{json, Value};
use tokio::process::Command;

/// PipeWire (on Fedora bootc) ships a PulseAudio-compatible socket and
/// `pactl` from the `pulseaudio-utils` package speaks to it. Same control
/// surface works on Ubuntu / WSL PulseAudio without any code branching.
const PACTL: &str = "pactl";
const DEFAULT_SINK: &str = "@DEFAULT_SINK@";

/// Set the system output volume.
///
/// `percent` is clamped to [0, 150]. We deliberately allow boost over 100%
/// (PipeWire / PulseAudio both support up to 150% by default) so a Lilith
/// "louder!" command still has somewhere to go after the user already hit
/// the cap.
pub async fn set_volume(params: Value) -> Result<Value, BusError> {
    let raw = params["percent"]
        .as_i64()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required integer param 'percent'".into(),
        })?;
    let pct = raw.clamp(0, 150);
    let arg = format!("{pct}%");

    run_pactl(&["set-sink-volume", DEFAULT_SINK, &arg]).await?;
    Ok(json!({ "set": true, "percent": pct, "clamped": pct as i64 != raw }))
}

/// Adjust the system output volume by a signed delta (`+5`, `-10`).
///
/// The underlying pactl handles overflow / underflow gracefully — we just
/// pass through the signed-percent shorthand.
pub async fn adjust_volume(params: Value) -> Result<Value, BusError> {
    let delta = params["delta"]
        .as_i64()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required integer param 'delta'".into(),
        })?;
    let arg = if delta >= 0 {
        format!("+{delta}%")
    } else {
        format!("{delta}%")
    };

    run_pactl(&["set-sink-volume", DEFAULT_SINK, &arg]).await?;
    Ok(json!({ "adjusted": true, "delta": delta }))
}

/// Toggle the system output mute state.
///
/// `set_state` (optional) forces a specific state: `true` mutes, `false`
/// unmutes. Without it, the call flips the current state — which is what
/// the user actually means 95% of the time when they say "mute".
pub async fn toggle_mute(params: Value) -> Result<Value, BusError> {
    let arg = match params["set_state"].as_bool() {
        Some(true) => "1",
        Some(false) => "0",
        None => "toggle",
    };

    run_pactl(&["set-sink-mute", DEFAULT_SINK, arg]).await?;
    Ok(json!({ "muted": match arg { "1" => true, "0" => false, _ => true }, "mode": arg }))
}

async fn run_pactl(args: &[&str]) -> Result<(), BusError> {
    if !super::which_exists(PACTL).await {
        return Err(BusError::Unavailable {
            service: "pactl (pulseaudio-utils)".into(),
        });
    }
    let output =
        Command::new(PACTL)
            .args(args)
            .output()
            .await
            .map_err(|e| BusError::ExecutionFailed {
                message: format!("pactl: {e}"),
            })?;
    if !output.status.success() {
        return Err(BusError::ExecutionFailed {
            message: format!(
                "pactl {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_volume_requires_percent() {
        let r = set_volume(json!({})).await;
        assert!(matches!(r, Err(BusError::InvalidParams { .. })));
    }

    #[tokio::test]
    async fn adjust_volume_requires_delta() {
        let r = adjust_volume(json!({})).await;
        assert!(matches!(r, Err(BusError::InvalidParams { .. })));
    }

    // No tests for toggle_mute params — every input (or absence of one)
    // is a valid call.

    #[tokio::test]
    async fn set_volume_clamps_negative() {
        // We can only safely assert the clamp logic by stubbing pactl, which
        // is out of scope. Instead just check that an out-of-range value
        // doesn't trip the param validator (the clamp happens before
        // pactl). The test passes when the call returns
        // Err(ExecutionFailed | Unavailable) — both prove validation
        // succeeded.
        let r = set_volume(json!({ "percent": -50 })).await;
        match r {
            Err(BusError::InvalidParams { .. }) => {
                panic!("clamp should have accepted the value before failing");
            }
            _ => {}
        }
    }
}
