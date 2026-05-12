//! Generic proxy from an Action Bus dispatch into a Jarvis SDK app's
//! `com.jarvis.app.<id>.Dispatch` method.
//!
//! Each SDK manifest action gets one registry entry whose handler is a
//! closure capturing the app's DBus service + path + action name. The
//! Action Bus's `build_registry` pulls `register_all` once at startup
//! and forgets about it.
//!
//! Apps own their own param validation: the bus passes whatever
//! `params` arrived as a serialised JSON string. The app returns a
//! string envelope, which we re-parse and forward as the
//! action result.

use crate::error::BusError;
use crate::registry::{HandlerFuture, Registry};
use jarvis_sdk_types::{load_manifests, Manifest};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;

/// Walk the SDK scan paths and register every action declared by every
/// valid manifest. Returns the number of actions added.
pub fn register_all(registry: &mut Registry, scan_paths: Vec<PathBuf>) -> usize {
    let manifests = load_manifests(&scan_paths);
    let mut count = 0;
    for m in manifests {
        let service = Arc::new(m.dbus_service());
        let path = Arc::new(m.dbus_path());
        for action in &m.actions {
            let registered_name = action.name.clone();
            let captured_name = action.name.clone();
            let service = service.clone();
            let path = path.clone();
            registry.register(
                registered_name,
                Arc::new(move |params: Value| {
                    let service = service.clone();
                    let path = path.clone();
                    let action_name = captured_name.clone();
                    Box::pin(dispatch(service, path, action_name, params)) as HandlerFuture
                }),
            );
            count += 1;
        }
        log_loaded(&m);
    }
    count
}

fn log_loaded(m: &Manifest) {
    tracing::info!(
        id = %m.app.id,
        actions = m.actions.len(),
        service = %m.dbus_service(),
        "Registered SDK app"
    );
}

async fn dispatch(
    service: Arc<String>,
    path: Arc<String>,
    action_name: String,
    params: Value,
) -> Result<Value, BusError> {
    let params_json = serde_json::to_string(&params).map_err(|e| BusError::InvalidParams {
        message: format!("could not serialise params: {e}"),
    })?;

    let conn = zbus::Connection::session()
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("session bus: {e}"),
        })?;

    let iface = service.as_str().to_string();
    let proxy = zbus::Proxy::new(&conn, service.as_str(), path.as_str(), iface.as_str())
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("{}: {e}", service.as_str()),
        })?;

    let response: String = proxy
        .call("Dispatch", &(action_name.as_str(), params_json.as_str()))
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("{}.Dispatch: {e}", service.as_str()),
        })?;

    let parsed: Value = serde_json::from_str(&response).map_err(|e| BusError::ExecutionFailed {
        message: format!("SDK app returned non-JSON: {e}"),
    })?;

    // Apps follow the same envelope shape as the bus itself.
    if let Some(err) = parsed.get("error") {
        let message = err
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("SDK app reported an error")
            .to_string();
        return Err(BusError::ExecutionFailed { message });
    }
    Ok(parsed.get("result").cloned().unwrap_or(parsed))
}
