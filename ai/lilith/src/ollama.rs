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

    /// Send a user message with conversation history + the available
    /// tools, parse the model's response.
    ///
    /// `history` is the recent session turns (oldest first). They're
    /// flattened into user/assistant message pairs so the model can
    /// resolve pronouns and follow-ups ("abre o gmail também" → uses
    /// the previous "abrir o navegador" as context).
    pub async fn chat(
        &self,
        user_text: &str,
        history: &[Turn],
        tools: &[Tool],
    ) -> Result<OllamaReply, LilithError> {
        let url = format!("{}/api/chat", self.host);
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

        let body = json!({
            "model": self.model,
            "messages": messages,
            "tools": ollama_tools_payload(tools),
            "stream": false
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

        let parsed: ChatResponse = resp
            .json()
            .await
            .map_err(|e| LilithError::OllamaInvalid(e.to_string()))?;

        Ok(OllamaReply::from(parsed))
    }
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

const SYSTEM_PROMPT: &str = "You are Lilith, the AI assistant inside Jarvis OS. \
You control the desktop by calling tools. When the user asks you to do something \
that maps to a tool, call the tool. When the user just chats or asks a question \
that doesn't need a tool, answer in plain text. Never invent tool names or fields \
that aren't in the provided tool list.";

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: ChatMessage,
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

impl From<ChatResponse> for OllamaReply {
    fn from(r: ChatResponse) -> Self {
        let calls = r
            .message
            .tool_calls
            .into_iter()
            .map(|c| ToolCall {
                action: c.function.name,
                params: c.function.arguments,
            })
            .collect();
        Self {
            text: r.message.content,
            tool_calls: calls,
        }
    }
}
