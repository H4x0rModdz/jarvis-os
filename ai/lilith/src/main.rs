mod audit;
mod bus_client;
mod error;
mod intent;
mod memory;
mod ollama;
mod persistent;
mod settings;
mod tools;

/// Hardcoded reply for `is_help_query`. Lists the namespaces of the
/// Action Bus catalogue, not every action — the user wants a tour,
/// not a man page. Phrased so the user can immediately copy a line
/// into the bar input.
const HELP_REPLY: &str = "\
Posso fazer isso aqui pra você:

• abrir / fechar apps   — \"abrir o navegador\", \"fechar o firefox\"
• instalar / remover    — \"instalar o gimp\" (via Flatpak)
• arquivos              — \"mover X para Y\", \"deletar X\"
• janelas               — focar / minimizar / maximizar / fechar / snap left|right
• áudio                 — \"volume 50\", \"mudo\", \"aumentar volume\"
• clipboard             — \"copiar X\", \"o que tem no clipboard\"
• screenshot            — \"tirar print\"
• navegador             — \"abrir https://…\"
• Wine / Proton         — \"rodar C:\\…\\app.exe\" (default ou prefix nomeado)
• notificações          — \"notifique X\"
• lembrar               — \"lembra que minha senha do roteador é 1234\" → memory.remember
• atualizar             — \"checar atualizações\", \"atualizar o sistema\"

Pergunte em português ou inglês — eu encadeio várias ações quando faz \
sentido (\"tira um print e abre no editor\").";

use audit::AuditLog;
use bus_client::BusClient;
use memory::{SessionMemory, Turn};
use ollama::{append_tool_step, build_initial_messages, OllamaClient};
use persistent::FactStore;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tools::{all_tools, ToolCall};

use zbus::{connection, interface, SignalContext};

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
    ///
    /// While the command runs, the daemon emits `PartialReply` signals
    /// carrying each token batch as it streams in from Ollama. The
    /// final return value carries the assembled text — clients that
    /// don't subscribe to the signal still see the full response.
    async fn command(
        &self,
        text: &str,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> String {
        let response = self.process(text, &ctx).await;
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

    /// Token batches as they stream in from Ollama. Multiple signals
    /// fire per Command() call; subscribers concatenate `chunk`
    /// values until the Command's return value lands. `step` is the
    /// 0-indexed chain step the chunk belongs to so multi-step
    /// chains (Phase 9) stay legible: text from step 0 vs. step 1
    /// can be rendered separately.
    #[zbus(signal)]
    async fn partial_reply(
        ctx: &SignalContext<'_>,
        step: u32,
        chunk: &str,
    ) -> zbus::Result<()>;

    /// Fired when the chain loop is about to dispatch a tool — lets
    /// the UI render "Capturando print…" / "Abrindo no editor…"
    /// inline before the tool actually finishes. `step` matches the
    /// step index on partial_reply so the shell can correlate.
    #[zbus(signal)]
    async fn chain_step(
        ctx: &SignalContext<'_>,
        step: u32,
        action: &str,
    ) -> zbus::Result<()>;
}

impl LilithService {
    async fn process(&self, text: &str, ctx: &SignalContext<'_>) -> Value {
        // 0. Capability discovery — short-circuits before the rule
        // path or the LLM. The response is a hardcoded listing of
        // what Lilith owns, in pt-BR. No Action Bus dispatch.
        if intent::is_help_query(text) {
            tracing::info!("Help intent matched");
            return self.respond_with_help(text).await;
        }

        // 1. Rule-based intent parser — fast path, deterministic.
        // No Ollama call → no streaming chunks. Subscribers that wait
        // for PartialReply still see the final Command() return.
        if let Some(call) = intent::parse(text) {
            tracing::info!(action = %call.action, "Rule matched");
            return self.dispatch_and_record(text, call).await;
        }

        // 2. Ollama path — multi-step tool chain. Each iteration:
        // ask Ollama, if it picks a tool we dispatch and feed the
        // result back, repeat until it returns a plain text response
        // or we hit the step cap. The history+chain loop is the
        // assistant pattern that makes "tira um screenshot e abre no
        // editor" actually work end-to-end.
        const HISTORY_TURNS: usize = 8;
        const MAX_STEPS: usize = 4;

        let history = self.memory.recent(HISTORY_TURNS);
        let mut messages = build_initial_messages(text, &history);

        // Per-step state we update as the loop progresses; final reply
        // pulls from these when we exit either with a text answer or
        // by hitting the step cap.
        let mut last_call: Option<ToolCall> = None;
        let mut last_response: Option<Value> = None;
        let mut last_step_reply = String::new();

        for step in 0..MAX_STEPS {
            // Per-step chunk channel: ollama writes tokens, the
            // forwarder task re-emits them as PartialReply signals
            // tagged with this step's index. Dropping the sender at
            // end of chat_messages closes the channel and the
            // forwarder finishes cleanly.
            let (chunk_tx, mut chunk_rx) =
                tokio::sync::mpsc::unbounded_channel::<String>();
            let ctx_for_forwarder = ctx.to_owned();
            let step_idx = step as u32;
            let forwarder = tokio::spawn(async move {
                while let Some(chunk) = chunk_rx.recv().await {
                    if let Err(e) = LilithService::partial_reply(
                        &ctx_for_forwarder,
                        step_idx,
                        &chunk,
                    )
                    .await
                    {
                        tracing::warn!(error = %e, "PartialReply emit failed");
                    }
                }
            });

            let reply_result = self
                .ollama
                .chat_messages(&messages, &all_tools(), Some(chunk_tx))
                .await;
            // Drain anything still in flight before we look at the
            // result; chunk_tx is already dropped by chat_messages
            // returning, so the forwarder's recv loop will hit None.
            let _ = forwarder.await;

            let reply = match reply_result {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(step, "Ollama unreachable: {e}");
                    let fallback = "Não entendi o comando. Diga, por exemplo: 'abrir vscode' ou 'fechar firefox'.";
                    self.audit.write(text, None, None, fallback).await;
                    self.memory.record(Turn {
                        user_text: text.into(),
                        tool_call: None,
                        action_response: None,
                        reply_text: fallback.into(),
                    });
                    return json!({ "reply": fallback, "action": null, "result": null });
                }
            };

            if reply.tool_calls.is_empty() {
                // Final answer. If empty, fall back to the last tool's
                // reply (the model decided the chain itself was the
                // answer); otherwise its text wins.
                let final_text = if !reply.text.trim().is_empty() {
                    reply.text
                } else if !last_step_reply.is_empty() {
                    last_step_reply.clone()
                } else {
                    "Não entendi o comando. Diga, por exemplo: 'abrir vscode' ou 'fechar firefox'.".into()
                };
                self.audit
                    .write(
                        text,
                        last_call.as_ref().map(|c| c.action.as_str()),
                        last_response.as_ref(),
                        &final_text,
                    )
                    .await;
                self.memory.record(Turn {
                    user_text: text.into(),
                    tool_call: last_call.clone(),
                    action_response: last_response.clone(),
                    reply_text: final_text.clone(),
                });
                return json!({
                    "reply": final_text,
                    "action": last_call.as_ref().map(|c| c.action.clone()),
                    "result": last_response,
                });
            }

            // Take the first tool call per step. Extras are rare
            // (qwen3 usually emits one) and the model gets to re-emit
            // them on the next round if it really wants both.
            let mut calls = reply.tool_calls;
            let extras = calls.len().saturating_sub(1);
            let call = calls.remove(0);
            if extras > 0 {
                tracing::info!(extras, "discarded extra tool_calls");
            }
            tracing::info!(step, action = %call.action, "Ollama selected tool");

            // Tell subscribers a tool is about to run before the
            // potentially-slow Action Bus call so the UI can render
            // "Capturando print…" before the result lands.
            if let Err(e) = Self::chain_step(ctx, step as u32, &call.action).await {
                tracing::warn!(error = %e, "ChainStep emit failed");
            }

            // Dispatch via the same helper that records per-step turns,
            // so cross-turn history (task #116) sees each step too.
            let step_value = self.dispatch_and_record(text, call.clone()).await;

            // Pull the per-step reply + response off the value we just
            // returned so we can both feed it back to Ollama and roll
            // it into the final return.
            last_call = Some(call.clone());
            last_response = step_value
                .get("result")
                .cloned()
                .filter(|v| !v.is_null());
            last_step_reply = step_value
                .get("reply")
                .and_then(|r| r.as_str())
                .unwrap_or_default()
                .to_string();

            append_tool_step(&mut messages, &call, last_response.as_ref());
        }

        // Hit the step cap. Most chains finish in 1–3 steps; if we got
        // here the model is probably loop-thrashing. Surface it.
        tracing::warn!(steps = MAX_STEPS, "step cap reached");
        json!({
            "reply": format!("(parei após {MAX_STEPS} passos) {last_step_reply}"),
            "action": last_call.as_ref().map(|c| c.action.clone()),
            "result": last_response,
        })
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

    /// Build the hardcoded capability listing returned to "/help",
    /// "ajuda", and "o que você sabe fazer". Records as a regular
    /// chat-style turn (no tool call) so the popup conversation
    /// view treats it like any other Lilith reply.
    async fn respond_with_help(&self, user_text: &str) -> Value {
        let reply = HELP_REPLY.to_string();
        self.audit.write(user_text, None, None, &reply).await;
        self.memory.record(Turn {
            user_text: user_text.into(),
            tool_call: None,
            action_response: None,
            reply_text: reply.clone(),
        });
        json!({ "reply": reply, "action": null, "result": null })
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
