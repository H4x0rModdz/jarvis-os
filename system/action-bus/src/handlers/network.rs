//! Network connection management via `nmcli`.
//!
//! Same pattern as the shell's NetworkBridge — wrapping the official
//! CLI gets us polkit + secret-agent handling for free, where driving
//! NetworkManager's DBus API directly would need six interfaces and a
//! custom agent. Lilith reaches these actions through the LLM
//! tool-call path ("conecte no <ssid>", "qual minha rede").
//!
//! All actions assume the running user has NetworkManager permission
//! (Fedora's default polkit rules give every logged-in user
//! connection management on their own session). On a hardened
//! deployment that revokes those rules, these actions return
//! ExecutionFailed with nmcli's stderr verbatim — the shell would hit
//! the same wall.

use crate::error::BusError;
use serde_json::{json, Value};
use tokio::process::Command;

const NMCLI: &str = "nmcli";

/// Force a fresh Wi-Fi scan + return the current list.
///
/// `--rescan yes` is the slow path (~3 s) — worth it because callers
/// invoking this action want fresh data, not the cached list.
pub async fn scan(_params: Value) -> Result<Value, BusError> {
    let out = run_nmcli(&[
        "-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY",
        "device", "wifi", "list", "--rescan", "yes",
    ])
    .await?;
    Ok(json!({ "networks": parse_wifi_list(&out) }))
}

/// Return the cached Wi-Fi list without a rescan. Cheap; use when
/// the caller wants "what's around right now" without the latency.
pub async fn list(_params: Value) -> Result<Value, BusError> {
    let out = run_nmcli(&[
        "-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY",
        "device", "wifi", "list",
    ])
    .await?;
    Ok(json!({ "networks": parse_wifi_list(&out) }))
}

/// Connect to `ssid`. `password` is empty for open networks. nmcli
/// stores a connection profile on success so subsequent boots
/// reconnect automatically — same behaviour the shell's panel gets.
pub async fn connect(params: Value) -> Result<Value, BusError> {
    let ssid = params["ssid"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'ssid'".into(),
        })?;
    let password = params["password"].as_str().unwrap_or("");

    let mut args: Vec<&str> = vec!["device", "wifi", "connect", ssid];
    if !password.is_empty() {
        args.push("password");
        args.push(password);
    }
    run_nmcli(&args).await?;
    Ok(json!({ "connected": true, "ssid": ssid }))
}

/// Drop the active Wi-Fi connection. The radio stays on.
pub async fn disconnect(_params: Value) -> Result<Value, BusError> {
    // Find the Wi-Fi device name from the active-connections list
    // first; passing the SSID itself to `nmcli device disconnect`
    // doesn't work (it takes a device, not a connection name).
    let active = run_nmcli(&[
        "-t", "-f", "NAME,TYPE,DEVICE",
        "connection", "show", "--active",
    ])
    .await?;
    let device = active
        .lines()
        .filter_map(|l| {
            let mut fields = l.splitn(3, ':');
            let _name = fields.next()?;
            let ty = fields.next()?;
            let dev = fields.next()?;
            if ty == "802-11-wireless" || ty == "wifi" {
                Some(dev.to_string())
            } else {
                None
            }
        })
        .next();
    let Some(dev) = device else {
        return Ok(json!({ "disconnected": false, "reason": "no active wifi" }));
    };
    run_nmcli(&["device", "disconnect", &dev]).await?;
    Ok(json!({ "disconnected": true, "device": dev }))
}

/// Toggle the Wi-Fi radio.
pub async fn set_enabled(params: Value) -> Result<Value, BusError> {
    let on = params["enabled"]
        .as_bool()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required bool param 'enabled'".into(),
        })?;
    run_nmcli(&["radio", "wifi", if on { "on" } else { "off" }]).await?;
    Ok(json!({ "enabled": on }))
}

/// Parse `nmcli -t` output, honouring backslash-escaping of `:` in
/// SSIDs. Same parser shape the NetworkBridge in the shell uses.
fn parse_wifi_list(out: &str) -> Vec<Value> {
    out.lines()
        .filter_map(|line| {
            let fields = split_nmcli_row(line);
            if fields.len() < 4 {
                return None;
            }
            let ssid = fields[1].clone();
            if ssid.trim().is_empty() {
                return None;
            }
            Some(json!({
                "ssid": ssid,
                "signal": fields[2].parse::<i32>().unwrap_or(0),
                "security": fields[3],
                "in_use": fields[0] == "*",
            }))
        })
        .collect()
}

fn split_nmcli_row(line: &str) -> Vec<String> {
    // Backslash-escape aware split on ':' — nmcli emits "Café\:WPA2"
    // when an SSID contains a colon.
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                current.push(next);
                chars.next();
            }
            continue;
        }
        if c == ':' {
            fields.push(std::mem::take(&mut current));
            continue;
        }
        current.push(c);
    }
    fields.push(current);
    fields
}

async fn run_nmcli(args: &[&str]) -> Result<String, BusError> {
    let output = Command::new(NMCLI)
        .args(args)
        .output()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("nmcli not available: {e}"),
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(BusError::ExecutionFailed {
            message: if stderr.is_empty() {
                format!("nmcli exited with {}", output.status)
            } else {
                stderr
            },
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
