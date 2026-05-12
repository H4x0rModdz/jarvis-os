use crate::error::BusError;
use serde_json::{json, Value};
use zbus::{Connection, Proxy};

const COMPAT_SERVICE: &str = "com.jarvis.Compat";
const COMPAT_PATH: &str = "/com/jarvis/Compat";
const COMPAT_IFACE: &str = "com.jarvis.Compat";

/// Run a Windows .exe through the Compat daemon. Params:
///   - `path`     (string, required)
///   - `args`     (array<string>, optional)
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

    let conn = Connection::session()
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("session bus: {e}"),
        })?;
    let proxy = Proxy::new(&conn, COMPAT_SERVICE, COMPAT_PATH, COMPAT_IFACE)
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Compat proxy: {e}"),
        })?;

    let response: String = proxy
        .call("RunExe", &(path, args.as_slice()))
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("Compat.RunExe: {e}"),
        })?;

    let parsed: Value = serde_json::from_str(&response).map_err(|e| BusError::ExecutionFailed {
        message: format!("Compat returned non-JSON: {e}"),
    })?;

    if parsed["started"].as_bool() == Some(true) {
        Ok(json!({
            "started": true,
            "pid": parsed["pid"].clone(),
            "path": path,
        }))
    } else {
        let reason = parsed["reason"]
            .as_str()
            .unwrap_or("unknown reason")
            .to_string();
        Err(BusError::ExecutionFailed { message: reason })
    }
}
