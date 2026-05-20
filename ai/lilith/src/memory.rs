use crate::tools::ToolCall;
use serde::Serialize;
use serde_json::Value;
use std::sync::Mutex;

/// Session memory — ephemeral. Cleared on `Reset()` or daemon restart.
///
/// v1: in-memory ring of the last N turns. v2 will persist to SQLite per
/// `.jarvis/architecture/ai-runtime.md`.
pub struct SessionMemory {
    turns: Mutex<Vec<Turn>>,
    capacity: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Turn {
    pub user_text: String,
    pub tool_call: Option<ToolCall>,
    pub action_response: Option<Value>,
    pub reply_text: String,
}

impl SessionMemory {
    pub fn new(capacity: usize) -> Self {
        Self {
            turns: Mutex::new(Vec::with_capacity(capacity)),
            capacity,
        }
    }

    pub fn record(&self, turn: Turn) {
        let mut t = self.turns.lock().unwrap();
        if t.len() == self.capacity {
            t.remove(0);
        }
        t.push(turn);
    }

    pub fn reset(&self) {
        self.turns.lock().unwrap().clear();
    }

    /// Snapshot the last `n` turns, oldest first. Cloned so the
    /// caller can release the mutex before any async work.
    pub fn recent(&self, n: usize) -> Vec<Turn> {
        let t = self.turns.lock().unwrap();
        let skip = t.len().saturating_sub(n);
        t.iter().skip(skip).cloned().collect()
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
