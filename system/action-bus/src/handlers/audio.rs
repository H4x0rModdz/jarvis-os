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
    Ok(json!({ "set": true, "percent": pct, "clamped": pct != raw }))
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

/// List every audio sink the daemon can see, plus which one is the
/// current default. Mirrors the data shape the shell's AudioBridge
/// surfaces so Lilith can talk about sinks the same way the
/// SettingsPanel renders them.
pub async fn list_sinks(_params: Value) -> Result<Value, BusError> {
    if !super::which_exists(PACTL).await {
        return Err(BusError::Unavailable {
            service: "pactl (pulseaudio-utils)".into(),
        });
    }
    let default_name = pactl_stdout(&["get-default-sink"])
        .await?
        .trim()
        .to_string();
    let list = pactl_stdout(&["list", "sinks"]).await?;
    let sinks = parse_sinks(&list, &default_name);
    Ok(json!({ "sinks": sinks, "default": default_name }))
}

/// Change the default sink AND migrate every running stream so
/// currently-playing audio actually follows the switch. Without
/// the stream move, `set-default-sink` only affects new streams —
/// which surprises users who expect their music to follow.
pub async fn set_default_sink(params: Value) -> Result<Value, BusError> {
    let sink = params["sink"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required string param 'sink'".into(),
        })?;
    run_pactl(&["set-default-sink", sink]).await?;
    // Move active streams. Failures here are non-fatal — the default
    // change already happened.
    if let Ok(streams) = pactl_stdout(&["list", "short", "sink-inputs"]).await {
        for row in streams.lines() {
            if let Some(id) = row.split('\t').next() {
                let id = id.trim();
                if !id.is_empty() {
                    let _ = run_pactl(&["move-sink-input", id, sink]).await;
                }
            }
        }
    }
    Ok(json!({ "default": sink }))
}

/// Parse `pactl list sinks` block format. Each Sink # starts a new
/// entry; Name / Description / Mute / Volume lines populate it. Same
/// shape the shell's AudioBridge uses — the two stay synced because
/// both inherit pactl's output as the source of truth.
fn parse_sinks(list: &str, default_name: &str) -> Vec<Value> {
    let mut sinks = Vec::new();
    let mut current = serde_json::Map::new();
    let flush = |cur: &mut serde_json::Map<String, Value>, acc: &mut Vec<Value>, default: &str| {
        if !cur.is_empty() {
            let name = cur
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            cur.insert("isDefault".into(), Value::Bool(name == default));
            acc.push(Value::Object(std::mem::take(cur)));
        }
    };
    for raw in list.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("Sink #") {
            flush(&mut current, &mut sinks, default_name);
            continue;
        }
        if let Some(rest) = line.strip_prefix("Name:") {
            current.insert("name".into(), Value::String(rest.trim().to_string()));
        } else if let Some(rest) = line.strip_prefix("Description:") {
            current.insert("description".into(), Value::String(rest.trim().to_string()));
        } else if let Some(rest) = line.strip_prefix("Mute:") {
            current.insert("mute".into(), Value::Bool(rest.trim() == "yes"));
        } else if let Some(rest) = line.strip_prefix("Volume:") {
            // First percentage match wins — pactl reports per-channel.
            let pct = rest
                .split_whitespace()
                .find_map(|tok| tok.trim_end_matches('%').parse::<i64>().ok())
                .unwrap_or(0);
            current.insert("volume".into(), Value::Number(pct.into()));
        }
    }
    flush(&mut current, &mut sinks, default_name);
    sinks
}

/// Helper: run pactl, return stdout on success. Mirrors run_pactl
/// but returns the captured stdout instead of swallowing it.
async fn pactl_stdout(args: &[&str]) -> Result<String, BusError> {
    let output = Command::new(PACTL)
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
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
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
        if let Err(BusError::InvalidParams { .. }) = r {
            panic!("clamp should have accepted the value before failing");
        }
    }
}
