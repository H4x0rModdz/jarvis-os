use crate::error::BusError;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type HandlerFuture = Pin<Box<dyn Future<Output = Result<Value, BusError>> + Send>>;
pub type HandlerFn = Arc<dyn Fn(Value) -> HandlerFuture + Send + Sync>;

pub struct Registry {
    handlers: HashMap<String, HandlerFn>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, action: impl Into<String>, handler: HandlerFn) {
        self.handlers.insert(action.into(), handler);
    }

    pub fn get(&self, action: &str) -> Option<HandlerFn> {
        self.handlers.get(action).cloned()
    }

    pub fn action_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.handlers.keys().cloned().collect();
        names.sort();
        names
    }
}

/// Convenience macro for registering async fn handlers without boilerplate.
#[macro_export]
macro_rules! register_handler {
    ($registry:expr, $action:expr, $handler:expr) => {
        $registry.register(
            $action,
            std::sync::Arc::new(|params: serde_json::Value| {
                Box::pin($handler(params))
                    as std::pin::Pin<
                        Box<
                            dyn std::future::Future<
                                    Output = Result<serde_json::Value, $crate::error::BusError>,
                                > + Send,
                        >,
                    >
            }),
        )
    };
}
