//! Bluetooth control via `bluetoothctl`.
//!
//! Same trade as the network module: wrapping the official CLI in
//! one-shot argv mode (bluez 5.66+) is ~10x less code than driving
//! BlueZ's DBus API directly. The shell's BluetoothBridge consumes
//! the exact same surface so the two stay in sync.
//!
//! V1 is just-works pairing only. Devices that need a numeric
//! passkey return an ExecutionFailed with bluetoothctl's stderr; a
//! future V2 will register an in-process agent for the prompt.

use crate::error::BusError;
use serde_json::{json, Value};
use tokio::process::Command;

const BLUETOOTHCTL: &str = "bluetoothctl";

/// 10 s discovery window. Returns the nearby + paired snapshot at
/// the end so the caller doesn't need to follow up with a list.
pub async fn scan(_params: Value) -> Result<Value, BusError> {
    run_btctl(&["--timeout", "10", "scan", "on"]).await?;
    let paired = list_paired_inner().await?;
    let nearby = list_nearby_inner(&paired).await?;
    Ok(json!({ "paired": paired, "nearby": nearby }))
}

/// Return paired devices with current connected state.
pub async fn list_paired(_params: Value) -> Result<Value, BusError> {
    Ok(json!({ "paired": list_paired_inner().await? }))
}

/// Discover nearby devices (cached, no scan). Use when the caller
/// just ran scan() and wants the latest list without paying the 10 s
/// discovery again.
pub async fn list_nearby(_params: Value) -> Result<Value, BusError> {
    let paired = list_paired_inner().await?;
    Ok(json!({ "nearby": list_nearby_inner(&paired).await? }))
}

/// pair → trust → connect, in sequence. Trust makes the device
/// auto-reconnect on next boot.
pub async fn pair(params: Value) -> Result<Value, BusError> {
    let mac = params["mac"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required string param 'mac'".into(),
        })?;
    run_btctl(&["pair", mac]).await?;
    let _ = run_btctl(&["trust", mac]).await; // best-effort
    run_btctl(&["connect", mac]).await?;
    Ok(json!({ "paired": true, "mac": mac }))
}

pub async fn unpair(params: Value) -> Result<Value, BusError> {
    let mac = params["mac"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required string param 'mac'".into(),
        })?;
    run_btctl(&["remove", mac]).await?;
    Ok(json!({ "unpaired": true, "mac": mac }))
}

pub async fn connect(params: Value) -> Result<Value, BusError> {
    let mac = params["mac"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required string param 'mac'".into(),
        })?;
    run_btctl(&["connect", mac]).await?;
    Ok(json!({ "connected": true, "mac": mac }))
}

pub async fn disconnect(params: Value) -> Result<Value, BusError> {
    let mac = params["mac"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required string param 'mac'".into(),
        })?;
    run_btctl(&["disconnect", mac]).await?;
    Ok(json!({ "disconnected": true, "mac": mac }))
}

pub async fn set_enabled(params: Value) -> Result<Value, BusError> {
    let on = params["enabled"]
        .as_bool()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required bool param 'enabled'".into(),
        })?;
    run_btctl(&["power", if on { "on" } else { "off" }]).await?;
    Ok(json!({ "enabled": on }))
}

// ── helpers ─────────────────────────────────────────────────────────

async fn list_paired_inner() -> Result<Vec<Value>, BusError> {
    // bluez 5.66+ accepts `devices Paired`; older versions need
    // `paired-devices`. Try the new form, fall back on non-zero.
    let raw = match run_btctl_out(&["devices", "Paired"]).await {
        Ok(s) => s,
        Err(_) => run_btctl_out(&["paired-devices"]).await?,
    };
    let mut paired = Vec::new();
    for line in raw.lines() {
        if let Some((mac, name)) = parse_device_line(line) {
            let connected = is_connected(&mac).await.unwrap_or(false);
            paired.push(json!({
                "mac": mac,
                "name": name,
                "connected": connected,
            }));
        }
    }
    Ok(paired)
}

async fn list_nearby_inner(paired: &[Value]) -> Result<Vec<Value>, BusError> {
    let raw = run_btctl_out(&["devices"]).await?;
    let paired_macs: std::collections::HashSet<String> = paired
        .iter()
        .filter_map(|v| v.get("mac").and_then(|m| m.as_str()).map(|s| s.to_string()))
        .collect();
    let mut nearby = Vec::new();
    for line in raw.lines() {
        if let Some((mac, name)) = parse_device_line(line) {
            if paired_macs.contains(&mac) {
                continue;
            }
            nearby.push(json!({ "mac": mac, "name": name }));
        }
    }
    Ok(nearby)
}

async fn is_connected(mac: &str) -> Result<bool, BusError> {
    let out = run_btctl_out(&["info", mac]).await?;
    Ok(out
        .lines()
        .map(str::trim)
        .any(|l| l.starts_with("Connected: yes")))
}

/// Parse "Device AA:BB:CC:DD:EE:FF Name with spaces" into (mac, name).
fn parse_device_line(line: &str) -> Option<(String, String)> {
    let t = line.trim();
    let rest = t.strip_prefix("Device ")?;
    let mut parts = rest.splitn(2, ' ');
    let mac = parts.next()?.to_string();
    let name = parts.next()?.to_string();
    // Sanity: MAC should be 17 chars + 5 colons.
    if mac.len() != 17 {
        return None;
    }
    Some((mac.to_uppercase(), name))
}

async fn run_btctl(args: &[&str]) -> Result<(), BusError> {
    if !super::which_exists(BLUETOOTHCTL).await {
        return Err(BusError::Unavailable {
            service: "bluetoothctl (bluez)".into(),
        });
    }
    let output = Command::new(BLUETOOTHCTL)
        .args(args)
        .output()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("bluetoothctl: {e}"),
        })?;
    if !output.status.success() {
        return Err(BusError::ExecutionFailed {
            message: format!(
                "bluetoothctl {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    Ok(())
}

async fn run_btctl_out(args: &[&str]) -> Result<String, BusError> {
    let output = Command::new(BLUETOOTHCTL)
        .args(args)
        .output()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("bluetoothctl: {e}"),
        })?;
    if !output.status.success() {
        return Err(BusError::ExecutionFailed {
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
