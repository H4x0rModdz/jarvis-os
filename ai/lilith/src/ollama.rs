use crate::error::LilithError;
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
    pub fn from_env() -> Self {
        let host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| DEFAULT_HOST.into());
        let model = std::env::var("LILITH_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into());
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .expect("reqwest client");
        Self { host, model, http }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Send a user message with the available tools and parse the model's response.
    pub async fn chat(&self, user_text: &str, tools: &[Tool]) -> Result<OllamaReply, LilithError> {
        let url = format!("{}/api/chat", self.host);
        let body = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": SYSTEM_PROMPT
                },
                {
                    "role": "user",
                    "content": user_text
                }
            ],
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
