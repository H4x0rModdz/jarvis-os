use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum LilithError {
    #[error("unknown intent: {0}")]
    UnknownIntent(String),

    #[error("ollama unreachable: {0}")]
    OllamaUnreachable(String),

    #[error("ollama returned invalid response: {0}")]
    OllamaInvalid(String),

    #[error("action bus error: {0}")]
    ActionBus(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
