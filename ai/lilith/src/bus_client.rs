use crate::error::LilithError;
use crate::tools::ToolCall;
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;
use zbus::{proxy, Connection};

/// Abstraction over the Action Bus so tests can swap in a recording
/// fake without standing up a DBus connection. Production
/// implementation is `BusClient`; test mocks live in the test module
/// in main.rs.
#[async_trait]
pub trait BusDispatcher: Send + Sync {
    async fn dispatch(&self, call: &ToolCall) -> Result<Value, LilithError>;
}

#[async_trait]
impl BusDispatcher for BusClient {
    async fn dispatch(&self, call: &ToolCall) -> Result<Value, LilithError> {
        BusClient::dispatch(self, call).await
    }
}

/// Generated proxy for `com.jarvis.ActionBus`.
#[proxy(
    interface = "com.jarvis.ActionBus",
    default_service = "com.jarvis.ActionBus",
    default_path = "/com/jarvis/ActionBus"
)]
trait ActionBus {
    fn dispatch(&self, action_json: &str) -> zbus::Result<String>;
    fn list_actions(&self) -> zbus::Result<Vec<String>>;
}

pub struct BusClient {
    proxy: ActionBusProxy<'static>,
    session_id: Uuid,
}

impl BusClient {
    pub async fn connect() -> Result<Self, LilithError> {
        let conn = Connection::session()
            .await
            .map_err(|e| LilithError::ActionBus(format!("connect: {e}")))?;
        let proxy = ActionBusProxy::new(&conn)
            .await
            .map_err(|e| LilithError::ActionBus(format!("proxy: {e}")))?;
        Ok(Self {
            proxy,
            session_id: Uuid::new_v4(),
        })
    }

    pub async fn dispatch(&self, call: &ToolCall) -> Result<Value, LilithError> {
        let request = json!({
            "action": call.action,
            "caller": { "type": "lilith" },
            "params": call.params,
            "session_id": self.session_id,
            "idempotency_key": null
        });
        let request_str = request.to_string();

        let response_str = self
            .proxy
            .dispatch(&request_str)
            .await
            .map_err(|e| LilithError::ActionBus(format!("dispatch: {e}")))?;

        serde_json::from_str(&response_str)
            .map_err(|e| LilithError::ActionBus(format!("response not JSON: {e}")))
    }
}
