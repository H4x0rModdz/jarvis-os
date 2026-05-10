use crate::error::BusError;
use serde_json::Value;

/// Window action handlers are stubs until the compositor module is built.
///
/// The compositor will register its own handlers via the Registry, overriding these.
/// These stubs return UNAVAILABLE so callers know the compositor isn't ready yet.

pub async fn focus(_params: Value) -> Result<Value, BusError> {
    compositor_unavailable("window.focus")
}

pub async fn minimize(_params: Value) -> Result<Value, BusError> {
    compositor_unavailable("window.minimize")
}

pub async fn maximize(_params: Value) -> Result<Value, BusError> {
    compositor_unavailable("window.maximize")
}

pub async fn close(_params: Value) -> Result<Value, BusError> {
    compositor_unavailable("window.close")
}

pub async fn move_window(_params: Value) -> Result<Value, BusError> {
    compositor_unavailable("window.move")
}

pub async fn resize(_params: Value) -> Result<Value, BusError> {
    compositor_unavailable("window.resize")
}

pub async fn snap_left(_params: Value) -> Result<Value, BusError> {
    compositor_unavailable("window.snap_left")
}

pub async fn snap_right(_params: Value) -> Result<Value, BusError> {
    compositor_unavailable("window.snap_right")
}

fn compositor_unavailable(action: &str) -> Result<Value, BusError> {
    Err(BusError::Unavailable {
        service: format!("{action}: compositor not yet available"),
    })
}
