mod audit;
mod bus_client;
mod error;
mod intent;
mod memory;
mod ollama;
mod persistent;
mod proactive;
mod settings;
mod signals;
mod tools;
mod turn_store;

use async_trait::async_trait;
use audit::AuditLog;
use bus_client::{BusClient, BusDispatcher};
use memory::{SessionMemory, Turn};
use ollama::{append_tool_step, build_initial_messages, Ollama, OllamaClient};
use persistent::FactStore;
use serde_json::{json, Value};
use signals::SignalSink;
use std::path::PathBuf;
use std::sync::Arc;
use tools::{all_tools, ToolCall};

use zbus::{connection, interface, SignalContext};

struct LilithService {
    bus: Arc<dyn BusDispatcher>,
    ollama: Arc<dyn Ollama>,
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
        // Wrap the zbus-injected signal context in a SignalSink so
        // process()'s loop can be tested without a live connection
        // (see signals.rs + the #[cfg(test)] module below).
        let sink: Arc<dyn SignalSink> = Arc::new(DbusSignalSink {
            ctx: ctx.to_owned(),
        });
        let response = self.process(text, sink).await;
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

/// Production SignalSink that forwards through zbus to the
/// `com.jarvis.Lilith` signals. Defined here (not in signals.rs) so
/// it can reference `LilithService`'s `#[zbus(signal)]` methods.
struct DbusSignalSink {
    ctx: SignalContext<'static>,
}

#[async_trait]
impl SignalSink for DbusSignalSink {
    async fn partial_reply(&self, step: u32, chunk: &str) {
        if let Err(e) = LilithService::partial_reply(&self.ctx, step, chunk).await {
            tracing::warn!(error = %e, "PartialReply emit failed");
        }
    }

    async fn chain_step(&self, step: u32, action: &str) {
        if let Err(e) = LilithService::chain_step(&self.ctx, step, action).await {
            tracing::warn!(error = %e, "ChainStep emit failed");
        }
    }
}

impl LilithService {
    async fn process(&self, text: &str, signals: Arc<dyn SignalSink>) -> Value {
        // 0. Capability discovery — short-circuits before the rule
        // path or the LLM. The response is a hardcoded listing of
        // what Lilith owns, in pt-BR. No Action Bus dispatch.
        if intent::is_help_query(text) {
            tracing::info!("Help intent matched");
            return self.respond_with_help(text).await;
        }

        // 0b. Math / unit conversion — shell out to numbat (Phase
        // 23.5). Keeps fast queries off the LLM path; numbat is a
        // pinned dependency so the result is deterministic.
        if let Some(expr) = intent::extract_calc_expression(text) {
            tracing::info!(%expr, "Calc intent matched");
            return self.respond_with_calc(text, &expr).await;
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
            // forwarder task re-emits them through the SignalSink
            // tagged with this step's index. Dropping the sender at
            // end of chat_messages closes the channel and the
            // forwarder finishes cleanly.
            let (chunk_tx, mut chunk_rx) =
                tokio::sync::mpsc::unbounded_channel::<String>();
            let sink_for_forwarder = signals.clone();
            let step_idx = step as u32;
            let forwarder = tokio::spawn(async move {
                while let Some(chunk) = chunk_rx.recv().await {
                    sink_for_forwarder.partial_reply(step_idx, &chunk).await;
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
            signals.chain_step(step as u32, &call.action).await;

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

    /// Build the capability listing returned to "/help", "ajuda", and
    /// "o que você sabe fazer". Generated from the live tool catalog
    /// (`tools::help_text`) so it always matches the actions Lilith
    /// actually exposes. Records as a regular chat-style turn so the
    /// popup conversation view treats it like any other Lilith reply.
    async fn respond_with_help(&self, user_text: &str) -> Value {
        let reply = tools::help_text();
        self.audit.write(user_text, None, None, &reply).await;
        self.memory.record(Turn {
            user_text: user_text.into(),
            tool_call: None,
            action_response: None,
            reply_text: reply.clone(),
        });
        json!({ "reply": reply, "action": null, "result": null })
    }

    /// Shell out to `numbat -e "<expr>"` and return the trimmed
    /// result. Records as a regular chat turn so the popup history
    /// keeps the question + answer pair. No Action Bus dispatch —
    /// pure local query, same shape as `respond_with_help`.
    ///
    /// numbat exits 0 on success with the value on stdout, non-zero
    /// with a diagnostic on stderr; we surface either as the reply
    /// so the user sees the error verbatim instead of a generic
    /// "não entendi".
    async fn respond_with_calc(&self, user_text: &str, expr: &str) -> Value {
        let reply = match tokio::process::Command::new("numbat")
            .args(["-e", expr])
            .output()
            .await
        {
            Ok(out) if out.status.success() => {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if s.is_empty() {
                    format!("(sem resultado de numbat para `{expr}`)")
                } else {
                    s
                }
            }
            Ok(out) => {
                let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
                if err.is_empty() {
                    format!("Não consegui calcular `{expr}`.")
                } else {
                    format!("Erro: {err}")
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "numbat spawn failed");
                format!("Numbat indisponível: {e}")
            }
        };

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
        // `memory.search` takes `query` (not `key`) and routes to the
        // turn store rather than the fact store — branch out before
        // the key-validation that the rest of the actions share.
        if call.action == "memory.search" {
            return Ok(self.handle_memory_search(call));
        }

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

    /// Search past turns by substring. Returns at most `limit`
    /// matches (default 5, capped at 50) newest-first, each with
    /// timestamp + user/reply text so the caller can quote back.
    /// Tool calls and action responses are intentionally dropped
    /// from the result — they bloat context for almost no signal
    /// when summarising a conversation.
    fn handle_memory_search(&self, call: &ToolCall) -> Value {
        let start = std::time::Instant::now();
        let query = call
            .params
            .get("query")
            .and_then(|q| q.as_str())
            .unwrap_or("")
            .trim();
        if query.is_empty() {
            return json!({
                "action": call.action,
                "status": "error",
                "error": { "code": "INVALID_PARAMS", "message": "missing 'query'" },
                "duration_ms": start.elapsed().as_millis() as u64,
            });
        }
        let limit = call
            .params
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(5)
            .min(50) as usize;

        let Some(store) = self.memory.store() else {
            return json!({
                "action": call.action,
                "status": "error",
                "error": { "code": "UNAVAILABLE", "message": "turn store not configured" },
                "duration_ms": start.elapsed().as_millis() as u64,
            });
        };

        let result = match store.search(query, limit) {
            Ok(hits) => {
                let matches: Vec<Value> = hits
                    .into_iter()
                    .map(|h| {
                        json!({
                            "ts": h.ts,
                            "user_text": h.turn.user_text,
                            "reply_text": h.turn.reply_text,
                        })
                    })
                    .collect();
                json!({
                    "action": call.action,
                    "status": "success",
                    "result": { "matches": matches },
                    "duration_ms": start.elapsed().as_millis() as u64,
                })
            }
            Err(e) => json!({
                "action": call.action,
                "status": "error",
                "error": { "code": "INTERNAL_ERROR", "message": e.to_string() },
                "duration_ms": start.elapsed().as_millis() as u64,
            }),
        };
        result
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

    let bus_concrete = BusClient::connect().await?;
    let bus: Arc<dyn BusDispatcher> = Arc::new(bus_concrete);
    let ollama_concrete = OllamaClient::from_env().await;
    tracing::info!("Ollama configured (model = {})", ollama_concrete.model());
    let ollama: Arc<dyn Ollama> = Arc::new(ollama_concrete);

    // Turn store sits next to the fact store. Same dir + WAL
    // settings; SessionMemory loads the most recent slice into
    // the cache at boot so a restart doesn't lose conversational
    // context.
    let turns_path = facts_path
        .parent()
        .map(|p| p.join("lilith.db"))
        .unwrap_or_else(|| std::path::PathBuf::from("./lilith.db"));
    tracing::info!("Turn store: {}", turns_path.display());
    let turn_store = Arc::new(turn_store::TurnStore::open(&turns_path)?);

    let memory = Arc::new(SessionMemory::with_store(32, turn_store.clone()));
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

// ── Test harness ─────────────────────────────────────────────────────
//
// Covers the pieces of LilithService that don't need a SignalContext:
// the help intent, dispatch_and_record's audit + memory bookkeeping,
// and the memory.* in-process tools. Full process() integration
// (which emits PartialReply/ChainStep via signal context) is left as
// Phase 13 work — it needs a SignalSink abstraction that lets tests
// pass a no-op sink. ADR-style trade-off recorded in module.md.

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use ollama::OllamaReply;
    use std::sync::Mutex as StdMutex;
    use tools::Tool;

    /// Scripted Ollama: hands out replies in order, fails the test
    /// if it runs out. Records the messages it was called with so
    /// tests can verify history flattening.
    struct MockOllama {
        replies: StdMutex<Vec<OllamaReply>>,
        seen_messages: StdMutex<Vec<Vec<Value>>>,
    }

    impl MockOllama {
        fn new(replies: Vec<OllamaReply>) -> Self {
            Self {
                replies: StdMutex::new(replies),
                seen_messages: StdMutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> usize {
            self.seen_messages.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl Ollama for MockOllama {
        async fn chat_messages(
            &self,
            messages: &[Value],
            _tools: &[Tool],
            _chunks: Option<tokio::sync::mpsc::UnboundedSender<String>>,
        ) -> Result<OllamaReply, error::LilithError> {
            self.seen_messages.lock().unwrap().push(messages.to_vec());
            let mut replies = self.replies.lock().unwrap();
            assert!(
                !replies.is_empty(),
                "MockOllama: ran out of scripted replies (tests should script enough)"
            );
            Ok(replies.remove(0))
        }
    }

    /// Records every dispatch + returns a configurable response per call.
    struct MockBus {
        responses: StdMutex<Vec<Value>>,
        seen_calls: StdMutex<Vec<ToolCall>>,
    }

    impl MockBus {
        fn new(responses: Vec<Value>) -> Self {
            Self {
                responses: StdMutex::new(responses),
                seen_calls: StdMutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<ToolCall> {
            self.seen_calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl BusDispatcher for MockBus {
        async fn dispatch(&self, call: &ToolCall) -> Result<Value, error::LilithError> {
            self.seen_calls.lock().unwrap().push(call.clone());
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                return Ok(json!({
                    "action": call.action,
                    "status": "success",
                    "duration_ms": 0,
                    "result": {},
                }));
            }
            Ok(responses.remove(0))
        }
    }

    fn temp_path(prefix: &str) -> PathBuf {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("jarvis-lilith-test-{prefix}-{}.db", ts))
    }

    fn build_service(
        ollama: Arc<dyn Ollama>,
        bus: Arc<dyn BusDispatcher>,
    ) -> LilithService {
        let facts_path = temp_path("facts");
        let _ = std::fs::remove_file(&facts_path);
        LilithService {
            bus,
            ollama,
            memory: Arc::new(SessionMemory::new(32)),
            facts: Arc::new(FactStore::open(&facts_path).unwrap()),
            audit: Arc::new(AuditLog::new(temp_path("audit").with_extension("log"))),
        }
    }

    #[tokio::test]
    async fn help_intent_short_circuits_without_ollama_or_bus() {
        let ollama = Arc::new(MockOllama::new(vec![]));
        let bus = Arc::new(MockBus::new(vec![]));
        let service = build_service(ollama.clone(), bus.clone());

        let resp = service.respond_with_help("/help").await;

        assert_eq!(resp["action"], Value::Null);
        let reply = resp["reply"].as_str().unwrap_or("");
        // Help text is now generated from the tool catalog: the
        // assertion targets concrete action names + the pt-BR
        // labels that the generator emits.
        assert!(reply.contains("aplicativos"));
        assert!(reply.contains("app.open"));
        assert!(reply.contains("encadeio"));
        // No DBus / Ollama calls — pure local response.
        assert_eq!(ollama.calls(), 0);
        assert!(bus.calls().is_empty());
    }

    #[tokio::test]
    async fn dispatch_and_record_writes_audit_and_memory() {
        let bus = Arc::new(MockBus::new(vec![json!({
            "action": "app.open",
            "status": "success",
            "duration_ms": 5,
            "result": { "launched": true },
        })]));
        let ollama = Arc::new(MockOllama::new(vec![]));
        let service = build_service(ollama, bus.clone());

        let call = ToolCall {
            action: "app.open".into(),
            params: json!({ "app": "firefox" }),
        };
        let resp = service.dispatch_and_record("abre firefox", call.clone()).await;

        assert_eq!(resp["action"], "app.open");
        assert_eq!(bus.calls().len(), 1);
        assert_eq!(bus.calls()[0].action, "app.open");
        // Memory captured the turn so cross-turn history (#116) works.
        assert_eq!(service.memory.recent(8).len(), 1);
    }

    #[tokio::test]
    async fn memory_tool_remember_recall_round_trip() {
        let ollama = Arc::new(MockOllama::new(vec![]));
        let bus = Arc::new(MockBus::new(vec![]));
        let service = build_service(ollama, bus.clone());

        // Remember
        let remember = ToolCall {
            action: "memory.remember".into(),
            params: json!({ "key": "router_pw", "value": "1234" }),
        };
        let r = service.handle_memory_tool(&remember).unwrap();
        assert_eq!(r["status"], "success");

        // Recall — should NOT hit the bus (memory.* is in-process).
        let recall = ToolCall {
            action: "memory.recall".into(),
            params: json!({ "key": "router_pw" }),
        };
        let r = service.handle_memory_tool(&recall).unwrap();
        assert_eq!(r["status"], "success");
        assert_eq!(r["result"]["value"], "1234");
        assert!(bus.calls().is_empty(), "memory.* must not touch the bus");
    }

    #[tokio::test]
    async fn memory_tool_recall_missing_returns_null_value() {
        let ollama = Arc::new(MockOllama::new(vec![]));
        let bus = Arc::new(MockBus::new(vec![]));
        let service = build_service(ollama, bus);

        let recall = ToolCall {
            action: "memory.recall".into(),
            params: json!({ "key": "nothing-here" }),
        };
        let r = service.handle_memory_tool(&recall).unwrap();
        assert_eq!(r["status"], "success");
        assert!(r["result"]["value"].is_null());
    }

    #[tokio::test]
    async fn memory_tool_rejects_empty_key() {
        let ollama = Arc::new(MockOllama::new(vec![]));
        let bus = Arc::new(MockBus::new(vec![]));
        let service = build_service(ollama, bus);

        let bad = ToolCall {
            action: "memory.remember".into(),
            params: json!({ "key": "", "value": "x" }),
        };
        let r = service.handle_memory_tool(&bad).unwrap();
        assert_eq!(r["status"], "error");
        assert_eq!(r["error"]["code"], "INVALID_PARAMS");
    }

    #[tokio::test]
    async fn memory_search_returns_matches_from_turn_store() {
        // The default `build_service` constructs SessionMemory
        // without a store — swap in a store-backed one before
        // running the search.
        let ollama = Arc::new(MockOllama::new(vec![]));
        let bus = Arc::new(MockBus::new(vec![]));
        let mut service = build_service(ollama, bus);
        let store = Arc::new(turn_store::TurnStore::in_memory().unwrap());
        service.memory = Arc::new(SessionMemory::with_store(32, store.clone()));

        // Three turns in the history, only one matches "gimp".
        service.memory.record(Turn {
            user_text: "instala o gimp".into(),
            tool_call: None,
            action_response: None,
            reply_text: "ok".into(),
        });
        service.memory.record(Turn {
            user_text: "abrir firefox".into(),
            tool_call: None,
            action_response: None,
            reply_text: "abrindo".into(),
        });

        let call = ToolCall {
            action: "memory.search".into(),
            params: json!({ "query": "gimp", "limit": 10 }),
        };
        let r = service.handle_memory_tool(&call).unwrap();
        assert_eq!(r["status"], "success");
        let matches = r["result"]["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["user_text"], "instala o gimp");
    }

    #[tokio::test]
    async fn memory_search_empty_query_is_invalid_params() {
        let ollama = Arc::new(MockOllama::new(vec![]));
        let bus = Arc::new(MockBus::new(vec![]));
        let service = build_service(ollama, bus);
        let call = ToolCall {
            action: "memory.search".into(),
            params: json!({ "query": "   " }),
        };
        let r = service.handle_memory_tool(&call).unwrap();
        assert_eq!(r["status"], "error");
        assert_eq!(r["error"]["code"], "INVALID_PARAMS");
    }

    #[tokio::test]
    async fn memory_search_without_store_returns_unavailable() {
        let ollama = Arc::new(MockOllama::new(vec![]));
        let bus = Arc::new(MockBus::new(vec![]));
        // Default build_service has no store attached → UNAVAILABLE.
        let service = build_service(ollama, bus);
        let call = ToolCall {
            action: "memory.search".into(),
            params: json!({ "query": "anything" }),
        };
        let r = service.handle_memory_tool(&call).unwrap();
        assert_eq!(r["status"], "error");
        assert_eq!(r["error"]["code"], "UNAVAILABLE");
    }

    #[test]
    fn help_query_matches_common_phrasings() {
        assert!(intent::is_help_query("/help"));
        assert!(intent::is_help_query("/ajuda"));
        assert!(intent::is_help_query("ajuda"));
        assert!(intent::is_help_query("o que você sabe fazer"));
        assert!(intent::is_help_query("O que voce sabe fazer?"));
        assert!(intent::is_help_query("what can you do"));
        // The "preciso de ajuda para X" false-positive guard:
        assert!(!intent::is_help_query("preciso de ajuda para abrir o navegador"));
        // Random non-help text:
        assert!(!intent::is_help_query("abrir o gmail"));
    }

    // ── Full process() integration tests ─────────────────────────────

    /// Records (step, payload) tuples instead of emitting DBus signals.
    /// Tests assert against these to verify the chain loop emitted
    /// the right sequence (which the production UI binds to).
    #[derive(Default)]
    struct RecordingSink {
        partials: StdMutex<Vec<(u32, String)>>,
        chain_steps: StdMutex<Vec<(u32, String)>>,
    }

    impl RecordingSink {
        fn chain_steps_seen(&self) -> Vec<(u32, String)> {
            self.chain_steps.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl SignalSink for RecordingSink {
        async fn partial_reply(&self, step: u32, chunk: &str) {
            self.partials.lock().unwrap().push((step, chunk.into()));
        }
        async fn chain_step(&self, step: u32, action: &str) {
            self.chain_steps.lock().unwrap().push((step, action.into()));
        }
    }

    fn tool_call_reply(action: &str, params: Value) -> OllamaReply {
        OllamaReply {
            text: String::new(),
            tool_calls: vec![ToolCall {
                action: action.into(),
                params,
            }],
        }
    }

    fn text_reply(text: &str) -> OllamaReply {
        OllamaReply {
            text: text.into(),
            tool_calls: Vec::new(),
        }
    }

    fn success_response(action: &str, result: Value) -> Value {
        json!({
            "action": action,
            "status": "success",
            "duration_ms": 1,
            "result": result,
        })
    }

    #[tokio::test]
    async fn process_help_path_short_circuits() {
        let ollama = Arc::new(MockOllama::new(vec![]));
        let bus = Arc::new(MockBus::new(vec![]));
        let sink: Arc<dyn SignalSink> = Arc::new(RecordingSink::default());
        let service = build_service(ollama.clone(), bus.clone());

        let resp = service.process("/help", sink.clone()).await;

        // The generated help text always contains the chain hint —
        // a more stable anchor than any individual action name.
        assert!(resp["reply"]
            .as_str()
            .unwrap_or("")
            .contains("encadeio"));
        assert_eq!(ollama.calls(), 0);
        assert!(bus.calls().is_empty());
    }

    #[tokio::test]
    async fn process_rule_path_dispatches_via_bus() {
        // "abrir firefox" matches the app.open rule in intent.rs.
        let ollama = Arc::new(MockOllama::new(vec![]));
        let bus = Arc::new(MockBus::new(vec![success_response(
            "app.open",
            json!({ "launched": true }),
        )]));
        let sink: Arc<dyn SignalSink> = Arc::new(RecordingSink::default());
        let service = build_service(ollama.clone(), bus.clone());

        let resp = service.process("abrir firefox", sink).await;

        assert_eq!(resp["action"], "app.open");
        // Rule path skips Ollama entirely.
        assert_eq!(ollama.calls(), 0);
        // Single bus dispatch.
        assert_eq!(bus.calls().len(), 1);
        assert_eq!(bus.calls()[0].action, "app.open");
    }

    #[tokio::test]
    async fn process_ollama_text_only() {
        // Ollama returns plain text with no tool calls — chat-only.
        let ollama = Arc::new(MockOllama::new(vec![text_reply("Tudo bem por aqui.")]));
        let bus = Arc::new(MockBus::new(vec![]));
        let sink_concrete = Arc::new(RecordingSink::default());
        let sink: Arc<dyn SignalSink> = sink_concrete.clone();
        let service = build_service(ollama.clone(), bus.clone());

        // A query that doesn't match any rule + isn't help.
        let resp = service.process("tudo bem com você?", sink).await;

        assert_eq!(resp["reply"], "Tudo bem por aqui.");
        assert_eq!(resp["action"], Value::Null);
        // One Ollama call, no bus dispatch, no chain_step signals.
        assert_eq!(ollama.calls(), 1);
        assert!(bus.calls().is_empty());
        assert!(sink_concrete.chain_steps_seen().is_empty());
    }

    #[tokio::test]
    async fn process_single_tool_call_then_text() {
        // First Ollama call: tool. Second: text wrap-up.
        let ollama = Arc::new(MockOllama::new(vec![
            tool_call_reply("browser.open", json!({ "url": "https://example.com" })),
            text_reply("Pronto, abri o site."),
        ]));
        let bus = Arc::new(MockBus::new(vec![success_response(
            "browser.open",
            json!({ "opened": true }),
        )]));
        let sink_concrete = Arc::new(RecordingSink::default());
        let sink: Arc<dyn SignalSink> = sink_concrete.clone();
        let service = build_service(ollama.clone(), bus.clone());

        let resp = service.process("abre example.com", sink).await;

        assert_eq!(resp["reply"], "Pronto, abri o site.");
        // Two Ollama calls (initial + post-tool wrap-up), one bus
        // dispatch, one chain_step signal at index 0.
        assert_eq!(ollama.calls(), 2);
        assert_eq!(bus.calls().len(), 1);
        assert_eq!(bus.calls()[0].action, "browser.open");
        let steps = sink_concrete.chain_steps_seen();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0], (0, "browser.open".to_string()));
    }

    #[tokio::test]
    async fn process_multi_step_chain() {
        // Screenshot → app.open chain — the canonical compound request.
        let ollama = Arc::new(MockOllama::new(vec![
            tool_call_reply("screenshot.capture", json!({})),
            tool_call_reply("app.open", json!({ "app": "feh" })),
            text_reply("Print salvo e aberto no visualizador."),
        ]));
        let bus = Arc::new(MockBus::new(vec![
            success_response("screenshot.capture", json!({ "path": "/tmp/shot.png" })),
            success_response("app.open", json!({ "launched": true })),
        ]));
        let sink_concrete = Arc::new(RecordingSink::default());
        let sink: Arc<dyn SignalSink> = sink_concrete.clone();
        let service = build_service(ollama.clone(), bus.clone());

        let resp = service
            .process("tira um print e abre no visualizador", sink)
            .await;

        assert_eq!(resp["reply"], "Print salvo e aberto no visualizador.");
        assert_eq!(ollama.calls(), 3);
        assert_eq!(bus.calls().len(), 2);
        let steps = sink_concrete.chain_steps_seen();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].1, "screenshot.capture");
        assert_eq!(steps[1].1, "app.open");
        // Each step gets its own session-memory Turn (Phase 9
        // promise) so cross-turn follow-ups see the full chain.
        assert_eq!(service.memory.recent(8).len(), 2);
    }

    #[tokio::test]
    async fn process_step_cap_hit() {
        // Ollama keeps emitting tool calls past the MAX_STEPS=4 cap.
        // Loop exits at step 4 with a "(parei após 4 passos)" reply.
        let mut replies = Vec::new();
        for _ in 0..6 {
            replies.push(tool_call_reply("app.open", json!({ "app": "firefox" })));
        }
        let ollama = Arc::new(MockOllama::new(replies));
        let bus = Arc::new(MockBus::new(
            (0..6)
                .map(|_| success_response("app.open", json!({ "launched": true })))
                .collect(),
        ));
        let sink_concrete = Arc::new(RecordingSink::default());
        let sink: Arc<dyn SignalSink> = sink_concrete.clone();
        let service = build_service(ollama.clone(), bus.clone());

        let resp = service.process("loop forever", sink).await;

        assert!(resp["reply"]
            .as_str()
            .unwrap_or("")
            .contains("parei após"));
        // Exactly 4 chain steps, even though the mock had 6 ready.
        assert_eq!(sink_concrete.chain_steps_seen().len(), 4);
        assert_eq!(bus.calls().len(), 4);
    }

    #[tokio::test]
    async fn process_ollama_error_falls_back() {
        // Custom MockOllama that always errors. Re-using MockOllama
        // would need an empty replies list — but that panics in the
        // mock. Easier to make a one-off error implementor here.
        struct ErrorOllama;
        #[async_trait]
        impl Ollama for ErrorOllama {
            async fn chat_messages(
                &self,
                _messages: &[Value],
                _tools: &[Tool],
                _chunks: Option<tokio::sync::mpsc::UnboundedSender<String>>,
            ) -> Result<OllamaReply, error::LilithError> {
                Err(error::LilithError::OllamaUnreachable("simulated".into()))
            }
        }

        let ollama = Arc::new(ErrorOllama);
        let bus = Arc::new(MockBus::new(vec![]));
        let sink: Arc<dyn SignalSink> = Arc::new(RecordingSink::default());
        let service = build_service(ollama, bus.clone());

        let resp = service.process("comando aleatório", sink).await;

        // Fallback path returns the canned "não entendi" reply.
        assert!(resp["reply"]
            .as_str()
            .unwrap_or("")
            .contains("Não entendi"));
        assert_eq!(resp["action"], Value::Null);
        assert!(bus.calls().is_empty());
    }
}
