use crate::error::LilithError;
use crate::memory::Turn;
use crate::tools::{ollama_tools_payload, Tool, ToolCall};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const DEFAULT_HOST: &str = "http://localhost:11434";
const DEFAULT_MODEL: &str = "qwen3:4b";
const REQUEST_TIMEOUT_SECS: u64 = 120;

#[derive(Clone)]
pub struct OllamaClient {
    host: String,
    model: String,
    http: reqwest::Client,
}

impl OllamaClient {
    /// Build a client. Resolves `model` through three layers, in order:
    ///   1. `lilith.model` from the Settings daemon (user-tunable via
    ///      the SettingsPanel — primary source of truth at runtime).
    ///   2. `LILITH_MODEL` env var (dev override, also used by the
    ///      service unit's drop-in workaround for low-RAM VMs).
    ///   3. The compiled-in default (`qwen3:4b`).
    ///
    /// Settings unreachable → silently falls through to env / default;
    /// Lilith always boots even when Settings is down.
    pub async fn from_env() -> Self {
        let host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| DEFAULT_HOST.into());
        let model = crate::settings::read_string("lilith.model")
            .await
            .or_else(|| std::env::var("LILITH_MODEL").ok())
            .unwrap_or_else(|| DEFAULT_MODEL.into());
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("reqwest client");
        Self { host, model, http }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Primary API. Streams the response from Ollama and forwards
    /// each token batch into `chunks` so the caller can echo them to
    /// the UI in real time. The returned `OllamaReply` accumulates
    /// the full text + any tool calls the model emitted by the end
    /// of the stream.
    ///
    /// Pass `None` for `chunks` to drain silently — convenient for
    /// callers that don't need streaming (tests, batch use).
    ///
    /// Ollama's `/api/chat` with `stream: true` returns NDJSON: each
    /// line is a `{ message: { content, tool_calls }, done }` object.
    /// `content` is the delta for that chunk (not the cumulative
    /// text). `tool_calls` usually only appear on the final `done`
    /// message, but we accumulate from any line that carries them
    /// for safety.
    pub async fn chat_messages(
        &self,
        messages: &[Value],
        tools: &[Tool],
        chunks: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    ) -> Result<OllamaReply, LilithError> {
        use futures_util::StreamExt;

        let url = format!("{}/api/chat", self.host);
        let body = json!({
            "model": self.model,
            "messages": messages,
            "tools": ollama_tools_payload(tools),
            "stream": true,
        });

        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| LilithError::OllamaUnreachable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(LilithError::OllamaInvalid(format!("HTTP {status}: {text}")));
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut accumulated_text = String::new();
        let mut tool_calls: Vec<OllamaToolCall> = Vec::new();

        while let Some(chunk_result) = stream.next().await {
            let bytes = chunk_result.map_err(|e| LilithError::OllamaInvalid(e.to_string()))?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            // NDJSON: parse complete lines, leave any partial trailer
            // in the buffer for the next chunk.
            while let Some(newline) = buffer.find('\n') {
                let line: String = buffer.drain(..=newline).collect();
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let parsed: ChatStreamChunk = match serde_json::from_str(line) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!(error = %e, line, "stream chunk parse failed");
                        continue;
                    }
                };
                if !parsed.message.content.is_empty() {
                    accumulated_text.push_str(&parsed.message.content);
                    if let Some(tx) = chunks.as_ref() {
                        let _ = tx.send(parsed.message.content);
                    }
                }
                tool_calls.extend(parsed.message.tool_calls);
                // `done: true` ends the stream — but reqwest already
                // signals EOF when the connection closes, so we don't
                // need to break here. Letting the outer loop see the
                // EOF keeps the parse robust to a trailing newline-
                // free `done` line.
            }
        }

        Ok(OllamaReply {
            text: accumulated_text,
            tool_calls: tool_calls
                .into_iter()
                .map(|c| ToolCall {
                    action: c.function.name,
                    params: c.function.arguments,
                })
                .collect(),
        })
    }

    /// Wrapper around `chat_messages` for the single-shot, non-
    /// streaming case: takes a user line + history, drains the
    /// stream silently, returns the accumulated response. Used by
    /// tests and the legacy callers that don't render incrementally.
    pub async fn chat(
        &self,
        user_text: &str,
        history: &[Turn],
        tools: &[Tool],
    ) -> Result<OllamaReply, LilithError> {
        let messages = build_initial_messages(user_text, history);
        self.chat_messages(&messages, tools, None).await
    }
}

/// Build the messages list for the first call of a turn: system prompt
/// + flattened history + current user text. The tool-chain loop in
/// `main.rs` extends this list with assistant/tool entries as it goes.
pub fn build_initial_messages(user_text: &str, history: &[Turn]) -> Vec<Value> {
    let mut messages: Vec<Value> = Vec::with_capacity(2 + history.len() * 2);
    messages.push(json!({ "role": "system", "content": SYSTEM_PROMPT }));
    for turn in history {
        messages.push(json!({ "role": "user", "content": turn.user_text }));
        messages.push(json!({
            "role": "assistant",
            "content": assistant_message_for(turn),
        }));
    }
    messages.push(json!({ "role": "user", "content": user_text }));
    messages
}

/// Append an assistant-with-tool-call message + the matching tool
/// message to an in-flight messages list. Used by the tool-chain loop
/// to feed each step's result back to the model.
pub fn append_tool_step(messages: &mut Vec<Value>, call: &ToolCall, response: Option<&Value>) {
    messages.push(json!({
        "role": "assistant",
        "content": "",
        "tool_calls": [{
            "function": {
                "name": call.action,
                "arguments": call.params,
            }
        }]
    }));
    let content = match response {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "{}".into()),
        None => "{\"status\":\"error\",\"error\":\"no response\"}".to_string(),
    };
    messages.push(json!({
        "role": "tool",
        "content": content,
    }));
}

/// Synthesise the assistant message for one historical Turn. When the
/// turn dispatched a tool we describe what happened so the model has
/// concrete context ("I opened the browser"); when it was a chat-only
/// reply we just pass through the original text.
fn assistant_message_for(turn: &Turn) -> String {
    match &turn.tool_call {
        Some(call) => {
            let params = if call.params.is_null() {
                String::new()
            } else {
                serde_json::to_string(&call.params).unwrap_or_default()
            };
            if params.is_empty() {
                format!(
                    "[I called {}; result: {}]",
                    call.action, turn.reply_text
                )
            } else {
                format!(
                    "[I called {}({}); result: {}]",
                    call.action, params, turn.reply_text
                )
            }
        }
        None => turn.reply_text.clone(),
    }
}

/// System prompt for the Ollama chat. Establishes identity, locale,
/// tone, the chain semantics that the multi-step loop relies on, and
/// the policy boundaries Jarvis OS expects of its assistant.
///
/// Kept terse on purpose: smaller models (qwen3:1.7b default) get
/// confused by long system prompts and start ignoring them. Every
/// line here earned its place.
const SYSTEM_PROMPT: &str = "\
You are Lilith, the assistant inside Jarvis OS — an AI-native desktop \
built on Fedora Atomic + labwc. The user runs you from a bar at the \
bottom of every screen.

Language: respond in Brazilian Portuguese unless the user clearly \
writes in another language. Stay concise — one or two sentences after \
a tool runs is usually enough.

Tools: the provided tool list is the complete set of effects you can \
trigger. Never invent tool names or fields. When a request maps to a \
tool, call it; when it's just chat or a question, reply in plain text.

Chaining: when a request needs more than one tool, call them ONE AT A \
TIME. You will see the result of each call before deciding the next \
step. Do not pre-plan the full chain in advance — react to what each \
tool returns. Stop calling tools when the user's goal is met; reply \
in text to confirm.

Safety: destructive actions (file.delete, app.uninstall) are \
permission-gated by the OS; you don't need to ask the user for \
confirmation twice. But if the user's intent is genuinely ambiguous \
(\"remove this\" without a target), ask before guessing.

Do not narrate what tool you are about to call. Do not say \"Let me \
call X for you\". Just call it. The shell shows the user that a tool \
ran; the only thing left for you to do is comment on the result.";

/// One line of the NDJSON stream Ollama emits for `/api/chat?stream`.
/// `message.content` is the *delta* — the new tokens since the last
/// chunk — not the cumulative text. `done` is true on the last line.
#[derive(Debug, Deserialize)]
struct ChatStreamChunk {
    message: ChatMessage,
    #[allow(dead_code)] // see comment in chat_messages re: EOF vs done
    #[serde(default)]
    done: bool,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Vec<OllamaToolCall>,
}

#[derive(Debug, Deserialize, Serialize)]
struct OllamaToolCall {
    function: OllamaFunction,
}

#[derive(Debug, Deserialize, Serialize)]
struct OllamaFunction {
    name: String,
    /// Ollama returns this as a JSON object inline (not a stringified JSON).
    arguments: Value,
}

#[derive(Debug)]
pub struct OllamaReply {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}
