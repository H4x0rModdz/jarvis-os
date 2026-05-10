use crate::action::{ActionRequest, ActionResponse};
use crate::audit::AuditLog;
use crate::error::BusError;
use crate::permission::PermissionChecker;
use crate::registry::Registry;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

pub struct ActionBus {
    registry: Arc<RwLock<Registry>>,
    permissions: PermissionChecker,
    audit: AuditLog,
}

impl ActionBus {
    pub fn new(registry: Registry, audit: AuditLog) -> Self {
        Self {
            registry: Arc::new(RwLock::new(registry)),
            permissions: PermissionChecker::new(),
            audit,
        }
    }

    pub async fn dispatch(&self, request: ActionRequest) -> ActionResponse {
        let start = Instant::now();
        let action = request.action.clone();

        tracing::info!(
            action = %action,
            caller = %request.caller,
            "Dispatching action"
        );

        let result = self.dispatch_inner(&request).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        let response = match result {
            Ok(value) => {
                tracing::info!(action = %action, duration_ms, "Action succeeded");
                ActionResponse::success(&action, value, duration_ms)
            }
            Err(e) => {
                tracing::warn!(action = %action, error = %e, duration_ms, "Action failed");
                ActionResponse::error(&action, e.error_code(), e.to_string(), duration_ms)
            }
        };

        self.audit.write(&request, &response).await;
        response
    }

    pub async fn list_actions(&self) -> Vec<String> {
        self.registry.read().await.action_names()
    }

    async fn dispatch_inner(&self, request: &ActionRequest) -> Result<serde_json::Value, BusError> {
        // 1. Permission check
        self.permissions.check(request).await?;

        // 2. Look up handler — clone Arc before releasing the read lock
        let handler = {
            let registry = self.registry.read().await;
            registry
                .get(&request.action)
                .ok_or_else(|| BusError::NotFound {
                    action: request.action.clone(),
                })?
            // read lock released here
        };

        // 3. Execute (no locks held during await)
        handler(request.params.clone()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Caller;
    use crate::registry::HandlerFn;
    use serde_json::json;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn make_bus() -> ActionBus {
        let mut registry = Registry::new();
        registry.register(
            "test.echo",
            Arc::new(|params| {
                Box::pin(async move { Ok(params) }) as crate::registry::HandlerFuture
            }),
        );
        registry.register(
            "test.fail",
            Arc::new(|_| {
                Box::pin(async move {
                    Err(BusError::ExecutionFailed {
                        message: "expected failure".into(),
                    })
                }) as crate::registry::HandlerFuture
            }),
        );
        ActionBus::new(
            registry,
            AuditLog::new(PathBuf::from("/tmp/jarvis-test-audit.log")),
        )
    }

    fn make_request(action: &str, params: serde_json::Value) -> ActionRequest {
        ActionRequest {
            action: action.to_owned(),
            caller: Caller::User,
            params,
            session_id: Uuid::new_v4(),
            idempotency_key: None,
        }
    }

    #[tokio::test]
    async fn dispatch_known_action_succeeds() {
        let bus = make_bus();
        let req = make_request("test.echo", json!({ "hello": "world" }));
        let resp = bus.dispatch(req).await;
        assert_eq!(resp.status, crate::action::ResponseStatus::Success);
        assert_eq!(resp.result, Some(json!({ "hello": "world" })));
    }

    #[tokio::test]
    async fn dispatch_unknown_action_returns_not_found() {
        let bus = make_bus();
        let req = make_request("nonexistent.action", json!({}));
        let resp = bus.dispatch(req).await;
        assert_eq!(resp.status, crate::action::ResponseStatus::Error);
        assert_eq!(resp.error.unwrap().code, "NOT_FOUND");
    }

    #[tokio::test]
    async fn dispatch_failing_handler_returns_error() {
        let bus = make_bus();
        let req = make_request("test.fail", json!({}));
        let resp = bus.dispatch(req).await;
        assert_eq!(resp.status, crate::action::ResponseStatus::Error);
    }
}
