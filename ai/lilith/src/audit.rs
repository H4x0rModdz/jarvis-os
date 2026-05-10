use chrono::Utc;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

#[derive(Serialize)]
struct AuditEntry<'a> {
    timestamp: String,
    user_text: &'a str,
    action: Option<&'a str>,
    response_status: Option<&'a str>,
    reply: &'a str,
}

pub struct AuditLog {
    path: PathBuf,
}

impl AuditLog {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub async fn write(
        &self,
        user_text: &str,
        action: Option<&str>,
        action_response: Option<&Value>,
        reply: &str,
    ) {
        let status = action_response
            .and_then(|v| v.get("status"))
            .and_then(|s| s.as_str());

        let entry = AuditEntry {
            timestamp: Utc::now().to_rfc3339(),
            user_text,
            action,
            response_status: status,
            reply,
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
            Ok(mut f) => {
                let _ = f.write_all(format!("{line}\n").as_bytes()).await;
            }
            Err(e) => {
                tracing::warn!("Failed to write audit log: {e}");
            }
        }
    }
}
