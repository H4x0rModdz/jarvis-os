mod audit;
mod bus_client;
mod error;
mod intent;
mod memory;
mod ollama;
mod persistent;
mod settings;
mod tools;

use audit::AuditLog;
use bus_client::BusClient;
use memory::{SessionMemory, Turn};
use ollama::OllamaClient;
use persistent::FactStore;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tools::{all_tools, ToolCall};

use zbus::{connection, interface};

struct LilithService {
    bus: Arc<BusClient>,
    ollama: OllamaClient,
    memory: Arc<SessionMemory>,
    facts: Arc<FactStore>,
    audit: Arc<AuditLog>,
}

#[interface(name = "com.jarvis.Lilith")]
impl LilithService {
    /// Process a natural-language command. Returns a JSON string:
    ///   { "reply": string, "action": string|null, "result": object|null }
    async fn command(&self, text: &str) -> String {
        let response = self.process(text).await;
        serde_json::to_string(&response).unwrap_or_else(|_| "{}".into())
    }

    /// Clear session memory. Persistent facts are unaffected — use `Forget` for those.
    async fn reset(&self) {
        self.memory.reset();
        tracing::info!("Session memory cleared");
    }

    /// Direct fact write — bypasses NLU. Returns the saved fact as JSON.
    async fn remember(&self, key: &str, value: &str) -> String {
        match self.facts.remember(key, value) {
            Ok(f) => serde_json::to_string(&f).unwrap_or_else(|_| "{}".into()),
            Err(e) => json!({ "error": e.to_string() }).to_string(),
        }
    }

    /// Direct fact read — returns `{ value: "..." }` or `{ value: null }`.
    async fn recall(&self, key: &str) -> String {
        match self.facts.recall(key) {
            Ok(Some(f)) => json!({ "value": f.value }).to_string(),
            Ok(None) => json!({ "value": null }).to_string(),
            Err(e) => json!({ "error": e.to_string() }).to_string(),
        }
    }

    /// Direct fact delete — returns `{ forgotten: bool }`.
    async fn forget(&self, key: &str) -> String {
        match self.facts.forget(key) {
            Ok(b) => json!({ "forgotten": b }).to_string(),
            Err(e) => json!({ "error": e.to_string() }).to_string(),
        }
    }

    /// List every stored fact as a JSON array.
    async fn list_facts(&self) -> String {
        match self.facts.list() {
            Ok(v) => serde_json::to_string(&v).unwrap_or_else(|_| "[]".into()),
            Err(e) => json!({ "error": e.to_string() }).to_string(),
        }
    }
}

impl LilithService {
    async fn process(&self, text: &str) -> Value {
        // 1. Rule-based intent parser — fast path, deterministic.
        if let Some(call) = intent::parse(text) {
            tracing::info!(action = %call.action, "Rule matched");
            return self.dispatch_and_record(text, call).await;
        }

        // 2. Fall back to Ollama for natural language. Feed the last
        // 8 turns so follow-up phrasing ("e o Gmail também", "agora
        // fecha tudo") resolves against real context instead of
        // running headfirst into a stateless LLM call.
        const HISTORY_TURNS: usize = 8;
        let history = self.memory.recent(HISTORY_TURNS);
        match self.ollama.chat(text, &history, &all_tools()).await {
            Ok(reply) => {
                if let Some(first) = reply.tool_calls.into_iter().next() {
                    tracing::info!(action = %first.action, "Ollama selected tool");
                    self.dispatch_and_record(text, first).await
                } else {
                    // Plain chat response — no tool needed.
                    let reply_text = if reply.text.trim().is_empty() {
                        "Não entendi o comando. Diga, por exemplo: 'abrir vscode' ou 'fechar firefox'.".to_string()
                    } else {
                        reply.text
                    };
                    self.audit.write(text, None, None, &reply_text).await;
                    self.memory.record(Turn {
                        user_text: text.into(),
                        tool_call: None,
                        action_response: None,
                        reply_text: reply_text.clone(),
                    });
                    json!({ "reply": reply_text, "action": null, "result": null })
                }
            }
            Err(e) => {
                tracing::warn!("Ollama unreachable: {e}");
                let reply =
                    "Não entendi o comando. Diga, por exemplo: 'abrir vscode' ou 'fechar firefox'.";
                self.audit.write(text, None, None, reply).await;
                self.memory.record(Turn {
                    user_text: text.into(),
                    tool_call: None,
                    action_response: None,
                    reply_text: reply.into(),
                });
                json!({ "reply": reply, "action": null, "result": null })
            }
        }
    }

    async fn dispatch_and_record(&self, user_text: &str, call: ToolCall) -> Value {
        let action_name = call.action.clone();

        // `memory.*` tools are Lilith-internal — they touch our own state,
        // not a system effect, so they bypass the Action Bus.
        let action_response = if action_name.starts_with("memory.") {
            self.handle_memory_tool(&call)
        } else {
            self.bus.dispatch(&call).await
        };

        let (reply, response_json) = match action_response {
            Ok(v) => {
                let status = v
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown");
                let reply = match status {
                    "success" => match v.get("result") {
                        Some(r) if action_name == "memory.recall" => {
                            match r.get("value").and_then(|x| x.as_str()) {
                                Some(val) => {
                                    format!("{}: {val}", call.params["key"].as_str().unwrap_or("?"))
                                }
                                None => format!(
                                    "Não tenho '{}' guardado.",
                                    call.params["key"].as_str().unwrap_or("?")
                                ),
                            }
                        }
                        Some(_) if action_name == "memory.remember" => {
                            format!("Guardado: {}", call.params["key"].as_str().unwrap_or("?"))
                        }
                        Some(r) if action_name == "memory.forget" => {
                            if r.get("forgotten")
                                .and_then(|b| b.as_bool())
                                .unwrap_or(false)
                            {
                                format!("Esquecido: {}", call.params["key"].as_str().unwrap_or("?"))
                            } else {
                                format!(
                                    "Não tinha '{}' guardado.",
                                    call.params["key"].as_str().unwrap_or("?")
                                )
                            }
                        }
                        _ => format!("Done: {action_name}"),
                    },
                    "error" => {
                        let msg = v
                            .get("error")
                            .and_then(|e| e.get("message"))
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown error");
                        format!("Failed: {msg}")
                    }
                    other => format!("{action_name}: {other}"),
                };
                (reply, Some(v))
            }
            Err(e) => (format!("Action Bus error: {e}"), None),
        };

        self.audit
            .write(
                user_text,
                Some(&action_name),
                response_json.as_ref(),
                &reply,
            )
            .await;

        self.memory.record(Turn {
            user_text: user_text.into(),
            tool_call: Some(call),
            action_response: response_json.clone(),
            reply_text: reply.clone(),
        });

        json!({
            "reply": reply,
            "action": action_name,
            "result": response_json,
        })
    }

    /// Execute a `memory.*` tool against the local fact store. Returns a value
    /// shaped exactly like an Action Bus response so the rest of the pipeline
    /// (`dispatch_and_record`) doesn't need a special case.
    fn handle_memory_tool(&self, call: &ToolCall) -> Result<Value, error::LilithError> {
        let key = call
            .params
            .get("key")
            .and_then(|k| k.as_str())
            .unwrap_or("");
        if key.is_empty() {
            return Ok(json!({
                "action": call.action,
                "status": "error",
                "error": { "code": "INVALID_PARAMS", "message": "missing 'key'" },
                "duration_ms": 0,
            }));
        }

        let start = std::time::Instant::now();
        let (status, result_value, error_value) = match call.action.as_str() {
            "memory.remember" => {
                let value = call
                    .params
                    .get("value")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if value.is_empty() {
                    (
                        "error",
                        None,
                        Some(json!({ "code": "INVALID_PARAMS", "message": "missing 'value'" })),
                    )
                } else {
                    match self.facts.remember(key, value) {
                        Ok(f) => ("success", Some(serde_json::to_value(&f).unwrap()), None),
                        Err(e) => (
                            "error",
                            None,
                            Some(json!({ "code": "INTERNAL_ERROR", "message": e.to_string() })),
                        ),
                    }
                }
            }
            "memory.recall" => match self.facts.recall(key) {
                Ok(Some(f)) => ("success", Some(json!({ "value": f.value })), None),
                Ok(None) => ("success", Some(json!({ "value": null })), None),
                Err(e) => (
                    "error",
                    None,
                    Some(json!({ "code": "INTERNAL_ERROR", "message": e.to_string() })),
                ),
            },
            "memory.forget" => match self.facts.forget(key) {
                Ok(b) => ("success", Some(json!({ "forgotten": b })), None),
                Err(e) => (
                    "error",
                    None,
                    Some(json!({ "code": "INTERNAL_ERROR", "message": e.to_string() })),
                ),
            },
            other => (
                "error",
                None,
                Some(
                    json!({ "code": "NOT_FOUND", "message": format!("unknown memory action: {other}") }),
                ),
            ),
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let mut response = json!({
            "action": call.action,
            "status": status,
            "duration_ms": duration_ms,
        });
        if let Some(r) = result_value {
            response["result"] = r;
        }
        if let Some(e) = error_value {
            response["error"] = e;
        }
        Ok(response)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jarvis_lilith=info".parse()?),
        )
        .init();

    tracing::info!("Starting Lilith");

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let audit_path = home.join(".jarvis/logs/lilith.log");
    let facts_path = home.join(".jarvis/lilith/facts.db");
    tracing::info!("Audit log: {}", audit_path.display());
    tracing::info!("Fact store: {}", facts_path.display());

    let bus = Arc::new(BusClient::connect().await?);
    let ollama = OllamaClient::from_env().await;
    tracing::info!("Ollama configured (model = {})", ollama.model());

    let memory = Arc::new(SessionMemory::new(32));
    let facts = Arc::new(FactStore::open(&facts_path)?);
    let audit = Arc::new(AuditLog::new(audit_path));

    let service = LilithService {
        bus,
        ollama,
        memory,
        facts,
        audit,
    };

    let _conn = connection::Builder::session()?
        .name("com.jarvis.Lilith")?
        .serve_at("/com/jarvis/Lilith", service)?
        .build()
        .await?;

    tracing::info!("Lilith ready on com.jarvis.Lilith");

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}
