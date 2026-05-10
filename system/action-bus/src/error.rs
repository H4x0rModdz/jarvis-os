use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum BusError {
    #[error("Permission denied for scope: {scope}")]
    PermissionDenied { scope: String },

    #[error("Action not found: {action}")]
    NotFound { action: String },

    #[error("Invalid parameters: {message}")]
    InvalidParams { message: String },

    #[error("Execution failed: {message}")]
    ExecutionFailed { message: String },

    #[error("User cancelled")]
    UserCancelled,

    #[error("Service unavailable: {service}")]
    Unavailable { service: String },
}

impl BusError {
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::PermissionDenied { .. } => "PERMISSION_DENIED",
            Self::NotFound { .. } => "NOT_FOUND",
            Self::InvalidParams { .. } => "INVALID_PARAMS",
            Self::ExecutionFailed { .. } => "INTERNAL_ERROR",
            Self::UserCancelled => "USER_CANCELLED",
            Self::Unavailable { .. } => "UNAVAILABLE",
        }
    }
}
