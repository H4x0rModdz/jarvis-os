use crate::tools::ToolCall;
use crate::turn_store::TurnStore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Session memory — the rolling N-turn cache the Ollama context
/// builder feeds. Optionally backed by a [`TurnStore`] so the
/// daemon survives restarts with history intact (Phase 25); when
/// no store is attached the cache is the only state and matches
/// the pre-Phase-25 behaviour.
///
/// `Reset()` clears the cache + (if present) the store. The store
/// is shared via Arc so other call sites — `memory.search` tool
/// in particular — can query it without going through the cache.
pub struct SessionMemory {
    turns: Mutex<Vec<Turn>>,
    capacity: usize,
    store: Option<Arc<TurnStore>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub user_text: String,
    pub tool_call: Option<ToolCall>,
    pub action_response: Option<Value>,
    pub reply_text: String,
}

impl SessionMemory {
    /// In-memory only — no persistence. Pre-Phase-25 behaviour.
    pub fn new(capacity: usize) -> Self {
        Self {
            turns: Mutex::new(Vec::with_capacity(capacity)),
            capacity,
            store: None,
        }
    }

    /// In-memory cache + SQLite persistence. Reads up to `capacity`
    /// recent turns from the store at init so the cache starts
    /// already warm — the Ollama context picks up where the last
    /// session left off.
    pub fn with_store(capacity: usize, store: Arc<TurnStore>) -> Self {
        let initial = store.recent(capacity).unwrap_or_else(|_| Vec::new());
        Self {
            turns: Mutex::new(initial),
            capacity,
            store: Some(store),
        }
    }

    pub fn record(&self, turn: Turn) {
        // Write-through to the store first so a panic during cache
        // update doesn't lose the turn. The store is the source of
        // truth; the cache is just a hot-path reader.
        if let Some(s) = &self.store {
            if let Err(e) = s.append(&turn) {
                tracing::warn!(error = %e, "turn_store append failed; cache only");
            }
        }
        let mut t = self.turns.lock().unwrap();
        if t.len() == self.capacity {
            t.remove(0);
        }
        t.push(turn);
    }

    /// Clear both the in-memory cache and the on-disk store. Used
    /// by `Reset()` on the DBus surface. The store's `clear` is a
    /// `trim_older_than(i64::MAX)` so we don't need a separate API.
    pub fn reset(&self) {
        self.turns.lock().unwrap().clear();
        if let Some(s) = &self.store {
            let _ = s.trim_older_than(i64::MAX);
        }
    }

    /// Snapshot the last `n` turns, oldest first. Cloned so the
    /// caller can release the mutex before any async work.
    pub fn recent(&self, n: usize) -> Vec<Turn> {
        let t = self.turns.lock().unwrap();
        let skip = t.len().saturating_sub(n);
        t.iter().skip(skip).cloned().collect()
    }

    /// Borrow the underlying store, if any. Lets the `memory.search`
    /// tool query the full history without going through the cache.
    pub fn store(&self) -> Option<Arc<TurnStore>> {
        self.store.clone()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.turns.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn turn(text: &str) -> Turn {
        Turn {
            user_text: text.into(),
            tool_call: None,
            action_response: None,
            reply_text: "ok".into(),
        }
    }

    #[test]
    fn ring_drops_oldest() {
        let mem = SessionMemory::new(2);
        mem.record(turn("first"));
        mem.record(turn("second"));
        mem.record(turn("third"));
        assert_eq!(mem.len(), 2);
    }

    #[test]
    fn reset_clears() {
        let mem = SessionMemory::new(10);
        mem.record(turn("hello"));
        mem.reset();
        assert_eq!(mem.len(), 0);
    }

    #[test]
    fn turn_serializes() {
        let t = Turn {
            user_text: "open vscode".into(),
            tool_call: Some(ToolCall {
                action: "app.open".into(),
                params: json!({ "app": "vscode" }),
            }),
            action_response: Some(json!({ "launched": true })),
            reply_text: "launched vscode".into(),
        };
        let s = serde_json::to_string(&t).unwrap();
        assert!(s.contains("app.open"));
    }
}
