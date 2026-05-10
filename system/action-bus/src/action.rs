use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub action: String,
    pub caller: Caller,
    pub params: serde_json::Value,
    pub session_id: Uuid,
    pub idempotency_key: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Caller {
    Lilith,
    User,
    Automation { id: String },
    App { id: String },
}

impl std::fmt::Display for Caller {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Caller::Lilith => write!(f, "lilith"),
            Caller::User => write!(f, "user"),
            Caller::Automation { id } => write!(f, "automation:{id}"),
            Caller::App { id } => write!(f, "app:{id}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResponse {
    pub action: String,
    pub status: ResponseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Success,
    Error,
    Pending,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: String,
    pub message: String,
}

impl ActionResponse {
    pub fn success(action: &str, result: serde_json::Value, duration_ms: u64) -> Self {
        Self {
            action: action.to_owned(),
            status: ResponseStatus::Success,
            result: Some(result),
            error: None,
            duration_ms,
        }
    }

    pub fn error(action: &str, code: &str, message: String, duration_ms: u64) -> Self {
        Self {
            action: action.to_owned(),
            status: ResponseStatus::Error,
            result: None,
            error: Some(ResponseError {
                code: code.to_owned(),
                message,
            }),
            duration_ms,
        }
    }
}
