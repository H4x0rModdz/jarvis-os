use crate::error::BusError;
use serde_json::{json, Value};
use zbus::{Connection, Proxy};

const COMPAT_SERVICE: &str = "com.jarvis.Compat";
const COMPAT_PATH: &str = "/com/jarvis/Compat";
const COMPAT_IFACE: &str = "com.jarvis.Compat";

async fn proxy() -> Result<Proxy<'static>, BusError> {
    let conn = Connection::session()
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("session bus: {e}"),
        })?;
    Proxy::new(&conn, COMPAT_SERVICE, COMPAT_PATH, COMPAT_IFACE)
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Compat proxy: {e}"),
        })
}

fn parse_response(response: String, expect_started: bool) -> Result<Value, BusError> {
    let parsed: Value = serde_json::from_str(&response).map_err(|e| BusError::ExecutionFailed {
        message: format!("Compat returned non-JSON: {e}"),
    })?;
    let success_key = if expect_started { "started" } else { "ok" };
    if parsed[success_key].as_bool() == Some(true) {
        Ok(parsed)
    } else {
        let reason = parsed["reason"]
            .as_str()
            .unwrap_or("unknown reason")
            .to_string();
        Err(BusError::ExecutionFailed { message: reason })
    }
}

/// Run a Windows .exe in the default prefix.
pub async fn run_exe(params: Value) -> Result<Value, BusError> {
    let path = params["path"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'path'".into(),
        })?;
    let args: Vec<String> = params["args"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let response: String = proxy()
        .await?
        .call("RunExe", &(path, args.as_slice()))
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Compat.RunExe: {e}"),
        })?;
    parse_response(response, true)
}

/// Run a Windows .exe via Proton-GE in a named prefix.
/// Proton-GE must be present at `~/.jarvis/proton-ge/` (or
/// `JARVIS_PROTON_DIR`). The daemon returns a clear "proton not
/// installed" reason when it isn't, with no auto-download — see
/// ADR 0017 for why.
pub async fn run_proton(params: Value) -> Result<Value, BusError> {
    let prefix = params["prefix"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'prefix'".into(),
        })?;
    let path = params["path"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'path'".into(),
        })?;
    let args: Vec<String> = params["args"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let response: String = proxy()
        .await?
        .call("RunProton", &(prefix, path, args.as_slice()))
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Compat.RunProton: {e}"),
        })?;
    parse_response(response, true)
}

/// Run a Windows .exe in a named prefix.
pub async fn run_exe_in(params: Value) -> Result<Value, BusError> {
    let prefix = params["prefix"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'prefix'".into(),
        })?;
    let path = params["path"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'path'".into(),
        })?;
    let args: Vec<String> = params["args"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let response: String = proxy()
        .await?
        .call("RunExeIn", &(prefix, path, args.as_slice()))
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Compat.RunExeIn: {e}"),
        })?;
    parse_response(response, true)
}

/// Create a named Wine prefix without running anything in it.
pub async fn create_prefix(params: Value) -> Result<Value, BusError> {
    let name = params["name"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'name'".into(),
        })?;

    let response: String = proxy()
        .await?
        .call("CreatePrefix", &(name,))
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Compat.CreatePrefix: {e}"),
        })?;
    parse_response(response, false)
}

/// Download + extract Proton-GE to ~/.jarvis/proton-ge/.
/// Long-running (300 MB); the daemon emits `InstallProgress` signals
/// during the fetch and pushes a single updating notification toast
/// so the user has a real progress indicator.
pub async fn install_proton(_params: Value) -> Result<Value, BusError> {
    let response: String = proxy()
        .await?
        .call("InstallProton", &())
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Compat.InstallProton: {e}"),
        })?;
    parse_response(response, false)
}

/// Enumerate every existing Wine prefix.
pub async fn list_prefixes(_params: Value) -> Result<Value, BusError> {
    let response: String = proxy()
        .await?
        .call("ListPrefixes", &())
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Compat.ListPrefixes: {e}"),
        })?;
    let parsed: Value = serde_json::from_str(&response).map_err(|e| BusError::ExecutionFailed {
        message: format!("Compat returned non-JSON: {e}"),
    })?;
    Ok(json!({ "prefixes": parsed["prefixes"].clone() }))
}

/// Snapshot of every running Wine/Proton child.
pub async fn list_running(_params: Value) -> Result<Value, BusError> {
    let response: String = proxy()
        .await?
        .call("ListRunning", &())
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Compat.ListRunning: {e}"),
        })?;
    let parsed: Value = serde_json::from_str(&response).map_err(|e| BusError::ExecutionFailed {
        message: format!("Compat returned non-JSON: {e}"),
    })?;
    Ok(json!({ "running": parsed["running"].clone() }))
}

/// SIGTERM a tracked child by pid. The daemon refuses pids it
/// doesn't track — only Wine/Proton children spawned through compat
/// can be terminated through this surface.
pub async fn terminate(params: Value) -> Result<Value, BusError> {
    let pid = params["pid"]
        .as_u64()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'pid' (u32)".into(),
        })? as u32;

    let response: String = proxy()
        .await?
        .call("Terminate", &(pid,))
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Compat.Terminate: {e}"),
        })?;
    parse_response(response, false)
}
