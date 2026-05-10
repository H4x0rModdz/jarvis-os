use crate::action::{ActionRequest, ActionResponse};
use chrono::Utc;
use serde::Serialize;
use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

#[derive(Serialize)]
struct AuditEntry<'a> {
    timestamp: String,
    action: &'a str,
    caller: String,
    status: &'a str,
    duration_ms: u64,
}

pub struct AuditLog {
    path: PathBuf,
}

impl AuditLog {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub async fn write(&self, request: &ActionRequest, response: &ActionResponse) {
        let status = match response.status {
            crate::action::ResponseStatus::Success => "success",
            crate::action::ResponseStatus::Error => "error",
            crate::action::ResponseStatus::Pending => "pending",
            crate::action::ResponseStatus::Cancelled => "cancelled",
        };

        let entry = AuditEntry {
            timestamp: Utc::now().to_rfc3339(),
            action: &request.action,
            caller: request.caller.to_string(),
            status,
            duration_ms: response.duration_ms,
        };

        let Ok(line) = serde_json::to_string(&entry) else {
            tracing::warn!("Failed to serialize audit entry");
            return;
        };

        if let Some(parent) = self.path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await
        {
            Ok(mut file) => {
                let _ = file.write_all(format!("{line}\n").as_bytes()).await;
            }
            Err(e) => {
                tracing::warn!("Failed to write audit log: {e}");
            }
        }
    }
}
