//! Jarvis Updater — first-boot setup daemon.
//!
//! Owns the bytes-on-the-wire fetch of assets that the ISO deliberately
//! does not bake in (Phase 1: the Lilith Ollama model). Surfaces progress
//! over DBus so `jarvis-shell` can render a splash without polling.
//!
//! See `system/updater/module.md` for the contract and ADR 0007 for the
//! rationale.

use anyhow::Context;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex as AsyncMutex;
use zbus::{connection, interface, SignalContext};

const DEFAULT_OLLAMA_HOST: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "qwen3:4b";

/// Throttle Progress signals to at most this often. Ollama emits NDJSON
/// lines very fast during chunked download — without throttling we'd
/// drown DBus and the QML progress bar would just flicker.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(200);

#[derive(Clone, Debug)]
struct Config {
    ollama_host: String,
    model: String,
}

impl Config {
    fn from_env() -> Self {
        Self {
            ollama_host: std::env::var("OLLAMA_HOST")
                .unwrap_or_else(|_| DEFAULT_OLLAMA_HOST.into()),
            model: std::env::var("LILITH_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into()),
        }
    }
}

#[derive(Debug, Serialize)]
struct CheckResult {
    model_present: bool,
    model: String,
    /// Phase 2 — always null for now. Reserved so callers can rely on the
    /// key existing.
    os_update_available: Option<bool>,
}

#[derive(Clone)]
struct UpdaterService {
    config: Config,
    http: reqwest::Client,
    /// Held by whichever task is currently running an Apply(). `Some` means
    /// busy. We can't trivially run two concurrent pulls anyway — Ollama
    /// itself serializes — and serialization on our side makes Progress
    /// ordering trivial.
    running: Arc<AsyncMutex<bool>>,
}

#[interface(name = "com.jarvis.Updater")]
impl UpdaterService {
    /// Inspect what is and isn't installed. No side effects.
    async fn check(&self) -> String {
        let model_present = self.is_model_present().await.unwrap_or(false);
        let result = CheckResult {
            model_present,
            model: self.config.model.clone(),
            os_update_available: None,
        };
        serde_json::to_string(&result).unwrap_or_else(|_| "{}".into())
    }

    /// Kick off the pull(s). Returns immediately with `{ started, reason? }`;
    /// the actual progress is reported via `Progress` and the terminal state
    /// via `Completed`.
    async fn apply(&self, #[zbus(signal_context)] ctx: SignalContext<'_>) -> String {
        {
            let mut busy = self.running.lock().await;
            if *busy {
                return json!({ "started": false, "reason": "busy" }).to_string();
            }
            *busy = true;
        }

        let svc = self.clone();
        let ctx_owned = ctx.to_owned();

        tokio::spawn(async move {
            let outcome = svc.run_apply(&ctx_owned).await;
            *svc.running.lock().await = false;

            let (success, message) = match outcome {
                Ok(()) => (true, format!("Model {} ready", svc.config.model)),
                Err(e) => (false, e.to_string()),
            };

            if let Err(e) =
                UpdaterService::completed(&ctx_owned, success, &message).await
            {
                tracing::warn!("Failed to emit Completed signal: {e}");
            }
        });

        json!({ "started": true }).to_string()
    }

    /// Streamed per-chunk progress.
    /// `stage` ∈ { "model.pull" }. `percent` is [0, 100] or -1 (indeterminate).
    #[zbus(signal)]
    async fn progress(
        ctx: &SignalContext<'_>,
        stage: &str,
        percent: i32,
        message: &str,
    ) -> zbus::Result<()>;

    /// Fires once per Apply() invocation.
    #[zbus(signal)]
    async fn completed(
        ctx: &SignalContext<'_>,
        success: bool,
        message: &str,
    ) -> zbus::Result<()>;
}

impl UpdaterService {
    fn new(config: Config) -> Self {
        let http = reqwest::Client::builder()
            // The pull is long-lived; no overall timeout. Connect timeout
            // keeps us snappy when Ollama is actually down.
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("reqwest client");
        Self {
            config,
            http,
            running: Arc::new(AsyncMutex::new(false)),
        }
    }

    /// Returns Ok(true) only if Ollama answers AND the model is in the list.
    /// Ok(false) for "Ollama answered, model absent". Err for "can't reach
    /// Ollama at all" — caller decides what that means.
    async fn is_model_present(&self) -> anyhow::Result<bool> {
        let url = format!("{}/api/tags", self.config.ollama_host);
        let resp = self
            .http
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .context("contact ollama /api/tags")?;
        if !resp.status().is_success() {
            anyhow::bail!("ollama /api/tags returned {}", resp.status());
        }
        let body: TagsResponse = resp.json().await.context("parse /api/tags")?;
        Ok(body
            .models
            .iter()
            .any(|m| model_matches(&m.name, &self.config.model)))
    }

    async fn run_apply(&self, ctx: &SignalContext<'_>) -> anyhow::Result<()> {
        // Re-check on entry so we don't redownload a model that arrived
        // between Check() and Apply().
        if self.is_model_present().await.unwrap_or(false) {
            tracing::info!(model = %self.config.model, "Model already present, nothing to do");
            return Ok(());
        }

        Self::progress(ctx, "model.pull", -1, "Contacting Ollama…").await?;

        let url = format!("{}/api/pull", self.config.ollama_host);
        let body = json!({ "model": self.config.model, "stream": true });

        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("POST /api/pull")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("ollama /api/pull returned {status}: {text}");
        }

        let mut stream = resp.bytes_stream();
        let mut buffer: Vec<u8> = Vec::with_capacity(8 * 1024);
        let mut last_emit = Instant::now()
            .checked_sub(PROGRESS_THROTTLE)
            .unwrap_or_else(Instant::now);

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("read /api/pull stream")?;
            buffer.extend_from_slice(&chunk);

            // Ollama emits one JSON object per line.
            while let Some(nl) = buffer.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = buffer.drain(..=nl).collect();
                let line_str = std::str::from_utf8(&line)
                    .context("ollama pull line not utf8")?
                    .trim();
                if line_str.is_empty() {
                    continue;
                }

                let event: PullEvent = serde_json::from_str(line_str)
                    .with_context(|| format!("parse ollama pull line: {line_str}"))?;

                if let Some(err) = event.error {
                    anyhow::bail!("ollama: {err}");
                }

                let throttled = last_emit.elapsed() < PROGRESS_THROTTLE;
                let terminal_event = matches!(event.status.as_deref(), Some("success"));

                if !throttled || terminal_event {
                    let (pct, msg) = event.to_progress();
                    Self::progress(ctx, "model.pull", pct, &msg).await?;
                    last_emit = Instant::now();
                }
            }
        }

        Ok(())
    }
}

/// Compare a stored Ollama model name (which may be tagged like
/// `qwen3:4b-instruct-q4_K_M`) against a configured tag. We treat
/// `<base>:<tag>` as matching if the base part is equal and either tags
/// match exactly or the requested tag is a prefix of the stored tag.
fn model_matches(stored: &str, requested: &str) -> bool {
    if stored == requested {
        return true;
    }
    let (stored_base, stored_tag) = stored.split_once(':').unwrap_or((stored, ""));
    let (req_base, req_tag) = requested.split_once(':').unwrap_or((requested, ""));
    stored_base == req_base && (stored_tag == req_tag || stored_tag.starts_with(req_tag))
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagEntry>,
}

#[derive(Debug, Deserialize)]
struct TagEntry {
    name: String,
}

#[derive(Debug, Deserialize)]
struct PullEvent {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    completed: Option<u64>,
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    error: Option<String>,
}

impl PullEvent {
    fn to_progress(&self) -> (i32, String) {
        let percent = match (self.completed, self.total) {
            (Some(c), Some(t)) if t > 0 => ((c as f64 / t as f64) * 100.0) as i32,
            _ => -1,
        };
        let msg = self
            .status
            .clone()
            .unwrap_or_else(|| "downloading…".to_string());
        (percent, msg)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jarvis_updater=info".parse()?),
        )
        .init();

    let config = Config::from_env();
    tracing::info!(
        ollama_host = %config.ollama_host,
        model = %config.model,
        "Starting Jarvis Updater"
    );

    let service = UpdaterService::new(config);

    let conn = connection::Builder::session()?
        .name("com.jarvis.Updater")?
        .serve_at("/com/jarvis/Updater", service.clone())?
        .build()
        .await?;

    tracing::info!("Updater ready on com.jarvis.Updater");

    // Self-trigger: if the configured asset is absent, immediately kick off
    // Apply() so the user sees a splash without anybody else having to call
    // us. If it's present, idle out — Phase 2 will replace this with a
    // periodic check.
    match service.is_model_present().await {
        Ok(true) => {
            tracing::info!("Model already present; updater idle");
        }
        Ok(false) => {
            tracing::info!("Model missing; auto-applying");
            // Acquire the bus path's signal context so the spawned task can
            // emit. zbus exposes this via the object server.
            let object_server = conn.object_server();
            let iface_ref = object_server
                .interface::<_, UpdaterService>("/com/jarvis/Updater")
                .await?;
            let ctx = iface_ref.signal_context().clone();
            let svc = service.clone();
            tokio::spawn(async move {
                let outcome = svc.run_apply(&ctx).await;
                let (success, message) = match outcome {
                    Ok(()) => (true, format!("Model {} ready", svc.config.model)),
                    Err(e) => (false, e.to_string()),
                };
                if let Err(e) = UpdaterService::completed(&ctx, success, &message).await {
                    tracing::warn!("Failed to emit Completed signal: {e}");
                }
            });
        }
        Err(e) => {
            tracing::warn!(error = %e, "Could not query Ollama; staying idle");
        }
    }

    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_matches_exact() {
        assert!(model_matches("qwen3:4b", "qwen3:4b"));
    }

    #[test]
    fn model_matches_with_quant_tag() {
        assert!(model_matches("qwen3:4b-instruct-q4_K_M", "qwen3:4b"));
    }

    #[test]
    fn model_does_not_match_different_base() {
        assert!(!model_matches("llama3:8b", "qwen3:4b"));
    }

    #[test]
    fn model_does_not_match_different_size() {
        assert!(!model_matches("qwen3:7b", "qwen3:4b"));
    }

    #[test]
    fn pull_event_progress_with_totals() {
        let ev = PullEvent {
            status: Some("downloading".into()),
            completed: Some(500),
            total: Some(1000),
            error: None,
        };
        let (pct, msg) = ev.to_progress();
        assert_eq!(pct, 50);
        assert_eq!(msg, "downloading");
    }

    #[test]
    fn pull_event_indeterminate_when_totals_missing() {
        let ev = PullEvent {
            status: Some("verifying".into()),
            completed: None,
            total: None,
            error: None,
        };
        let (pct, _) = ev.to_progress();
        assert_eq!(pct, -1);
    }
}
