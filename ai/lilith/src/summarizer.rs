//! Auto-summary loop.
//!
//! When the TurnStore exceeds a threshold, the oldest batch of
//! turns is fed to Ollama with a "compress this conversation"
//! prompt, the result is written to the `summaries` table, and
//! the source turns are deleted. The Ollama context builder
//! (#174) prepends the most recent summary so the assistant
//! keeps long-term context even after the raw rows are pruned.
//!
//! Why a periodic task instead of triggering on each `record()`:
//!
//! - Summarising on every turn flush would call Ollama dozens of
//!   times per minute during a busy session.
//! - The cost (one LLM round-trip every ~5 min when the user is
//!   active) is acceptable.
//! - The user shouldn't see a multi-second pause on their reply
//!   because the daemon happened to cross the threshold mid-call.
//!
//! Tuning:
//!
//!   `TRIGGER_AT`     — minimum live-turn count before we compress
//!   `BATCH_SIZE`     — how many oldest turns to fold per pass
//!   `TICK`           — how often the loop checks the threshold
//!
//! Failures (Ollama unreachable, schema error) are logged and the
//! next tick retries.

use crate::ollama::Ollama;
use crate::turn_store::TurnStore;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// When this many or more turns sit in the live table, compress
/// the oldest BATCH_SIZE into one summary row.
pub const TRIGGER_AT: i64 = 200;
pub const BATCH_SIZE: usize = 100;

/// Interval the loop wakes up to check. 5 min keeps the daemon
/// quiet during a typing session — the threshold is well above
/// what a chatty hour produces, so we don't need finer cadence.
pub const TICK: Duration = Duration::from_secs(5 * 60);

/// Spawn the background summarisation task. Held alive by the
/// runtime; the only exit is daemon shutdown.
pub fn spawn_loop(store: Arc<TurnStore>, ollama: Arc<dyn Ollama>) {
    tokio::spawn(async move {
        // Skip the first immediate tick — boot races with Ollama
        // model load, no point thrashing the LLM before it's
        // ready.
        let mut interval = tokio::time::interval(TICK);
        interval.tick().await;
        loop {
            interval.tick().await;
            if let Err(e) = step(&store, ollama.as_ref()).await {
                tracing::warn!(error = %e, "summarizer step failed");
            }
        }
    });
}

/// One pass: check threshold, summarise if needed, delete source
/// rows. Public for tests.
pub async fn step(
    store: &TurnStore,
    ollama: &dyn Ollama,
) -> Result<bool, crate::error::LilithError> {
    let total = store.count()?;
    if total < TRIGGER_AT {
        return Ok(false);
    }
    let batch = store.oldest(BATCH_SIZE)?;
    if batch.is_empty() {
        return Ok(false);
    }
    let ts_from = batch.first().map(|t| t.ts).unwrap_or(0);
    let ts_to = batch.last().map(|t| t.ts).unwrap_or(0);
    let last_id = batch.last().map(|t| t.id).unwrap_or(0);

    let transcript = render_transcript(&batch);
    let messages = vec![
        json!({
            "role": "system",
            "content":
                "Você é um sumarizador. Recebe um trecho de conversa entre \
                 o usuário e a Lilith. Devolva um parágrafo de 3-6 linhas \
                 em português listando os tópicos cobertos, decisões \
                 tomadas, e fatos sobre o usuário que merecem ser \
                 lembrados. NÃO inclua data, hora, nem repita o trecho \
                 literal. Seja conciso."
        }),
        json!({ "role": "user", "content": transcript }),
    ];

    let reply = ollama.chat_messages(&messages, &[], None).await?;
    let text = reply.text.trim();
    if text.is_empty() {
        tracing::warn!("summarizer: ollama returned empty text; skipping batch");
        return Ok(false);
    }

    store.record_summary(ts_from, ts_to, batch.len() as i64, text)?;
    store.delete_through(last_id)?;
    tracing::info!(
        turns = batch.len(),
        last_id,
        "summarizer: compressed batch into one summary row"
    );
    Ok(true)
}

/// Flatten a batch of turns into a single string the LLM can read.
/// Drops tool-call JSON since the assistant only needs the
/// human-readable arc, not the wire-level dispatch detail.
fn render_transcript(batch: &[crate::turn_store::StoredTurn]) -> String {
    let mut out = String::new();
    for st in batch {
        let user = st.turn.user_text.trim();
        let reply = st.turn.reply_text.trim();
        if !user.is_empty() {
            out.push_str("Usuário: ");
            out.push_str(user);
            out.push('\n');
        }
        if !reply.is_empty() {
            out.push_str("Lilith: ");
            out.push_str(reply);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Turn;
    use crate::ollama::OllamaReply;
    use crate::tools::Tool;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Local mock — captures the prompt the summarizer sends and
    /// returns a scripted reply. Doesn't talk to a real model.
    struct CapturingOllama {
        reply: String,
        prompts: Mutex<Vec<Vec<serde_json::Value>>>,
    }
    impl CapturingOllama {
        fn new(reply: &str) -> Self {
            Self {
                reply: reply.into(),
                prompts: Mutex::new(Vec::new()),
            }
        }
        fn captured(&self) -> Vec<Vec<serde_json::Value>> {
            self.prompts.lock().unwrap().clone()
        }
    }
    #[async_trait]
    impl Ollama for CapturingOllama {
        async fn chat_messages(
            &self,
            messages: &[serde_json::Value],
            _tools: &[Tool],
            _chunks: Option<tokio::sync::mpsc::UnboundedSender<String>>,
        ) -> Result<OllamaReply, crate::error::LilithError> {
            self.prompts.lock().unwrap().push(messages.to_vec());
            Ok(OllamaReply {
                text: self.reply.clone(),
                tool_calls: vec![],
            })
        }
    }

    fn fill_turns(store: &TurnStore, n: usize, prefix: &str) {
        for i in 0..n {
            store
                .append(&Turn {
                    user_text: format!("{prefix} q{i}"),
                    tool_call: None,
                    action_response: None,
                    reply_text: format!("{prefix} a{i}"),
                })
                .unwrap();
        }
    }

    #[tokio::test]
    async fn step_noop_when_below_threshold() {
        let store = Arc::new(TurnStore::in_memory().unwrap());
        fill_turns(&store, 10, "x");
        let ollama = Arc::new(CapturingOllama::new(""));
        let did_work = step(&store, ollama.as_ref()).await.unwrap();
        assert!(!did_work);
        assert!(
            ollama.captured().is_empty(),
            "should not have called Ollama"
        );
        assert_eq!(store.count().unwrap(), 10);
    }

    #[tokio::test]
    async fn step_compresses_when_above_threshold() {
        let store = Arc::new(TurnStore::in_memory().unwrap());
        // Just over the threshold so one batch fires.
        fill_turns(&store, (TRIGGER_AT as usize) + 5, "y");
        let ollama = Arc::new(CapturingOllama::new("o usuário pediu para instalar X"));
        let did_work = step(&store, ollama.as_ref()).await.unwrap();
        assert!(did_work);
        // Original count was TRIGGER_AT+5; BATCH_SIZE turns were
        // deleted; one summary row added.
        assert_eq!(store.count().unwrap(), (TRIGGER_AT + 5) - BATCH_SIZE as i64);
        let s = store.latest_summary().unwrap().expect("summary recorded");
        assert_eq!(s.turn_count, BATCH_SIZE as i64);
        assert!(s.text.contains("instalar X"));
        // Ollama prompt contained the rendered transcript (the
        // 'Usuário:' marker is present in the user-role message).
        let captured = ollama.captured();
        assert_eq!(captured.len(), 1);
        let user_msg = &captured[0][1];
        assert_eq!(user_msg["role"], "user");
        assert!(user_msg["content"].as_str().unwrap().contains("Usuário:"));
    }

    #[tokio::test]
    async fn step_skips_when_ollama_returns_empty() {
        let store = Arc::new(TurnStore::in_memory().unwrap());
        fill_turns(&store, (TRIGGER_AT as usize) + 5, "z");
        let ollama = Arc::new(CapturingOllama::new("   "));
        let did_work = step(&store, ollama.as_ref()).await.unwrap();
        assert!(!did_work, "empty Ollama text should skip recording");
        // No summary written, no turns deleted.
        assert!(store.latest_summary().unwrap().is_none());
        assert_eq!(store.count().unwrap(), TRIGGER_AT + 5);
    }
}
