use crate::error::BusError;
use serde_json::Value;

pub async fn switch(_params: Value) -> Result<Value, BusError> {
    compositor_unavailable("workspace.switch")
}

pub async fn move_window(_params: Value) -> Result<Value, BusError> {
    compositor_unavailable("workspace.move_window")
}

pub async fn create(_params: Value) -> Result<Value, BusError> {
    compositor_unavailable("workspace.create")
}

fn compositor_unavailable(action: &str) -> Result<Value, BusError> {
    Err(BusError::Unavailable {
        service: format!("{action}: compositor not yet available"),
    })
}
