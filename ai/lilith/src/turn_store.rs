//! Persistent turn history for Lilith.
//!
//! Each row records one assistant turn — what the user asked, what
//! tool Lilith ended up calling (if any), the tool's response, and
//! the final reply text. Lives at `~/.jarvis/lilith.db` alongside
//! the facts store; same WAL settings.
//!
//! The current `SessionMemory` ring buffer becomes a cache over
//! this — we still serve `recent(n)` from memory for speed but the
//! daemon now survives restarts with history intact. Search lands
//! as a Lilith tool (#165) so the assistant can answer "o que
//! falamos sobre X" against the full log instead of only the
//! in-process slice.

use crate::error::LilithError;
use crate::memory::Turn;
use crate::tools::ToolCall;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

pub struct TurnStore {
    conn: Mutex<Connection>,
}

/// Search-result row — `Turn` plus the surrounding metadata search
/// callers want (the rowid + the timestamp). Kept separate from
/// `Turn` so the in-memory path doesn't drag a timestamp around it
/// doesn't need.
#[derive(Debug, Clone, Serialize)]
pub struct StoredTurn {
    pub id: i64,
    pub ts: i64,
    pub turn: Turn,
}

impl TurnStore {
    pub fn open(path: &Path) -> Result<Self, LilithError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)
            .map_err(|e| LilithError::Io(std::io::Error::other(e.to_string())))?;
        // WAL keeps reads fast while one writer is appending — matches
        // the notifications/facts store pattern.
        conn.execute_batch("PRAGMA journal_mode = WAL;")
            .map_err(sql_err)?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self, LilithError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| LilithError::Io(std::io::Error::other(e.to_string())))?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn init_schema(conn: &Connection) -> Result<(), LilithError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS turns (
                id                    INTEGER PRIMARY KEY AUTOINCREMENT,
                ts                    INTEGER NOT NULL,
                user_text             TEXT    NOT NULL,
                tool_call_json        TEXT,
                action_response_json  TEXT,
                reply_text            TEXT    NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_turns_ts ON turns(ts DESC);",
        )
        .map_err(sql_err)?;
        Ok(())
    }

    /// Append one turn. Returns the assigned rowid + the timestamp
    /// (UNIX epoch seconds) so the caller can keep a richer in-memory
    /// cache if it wants.
    pub fn append(&self, turn: &Turn) -> Result<(i64, i64), LilithError> {
        let ts = chrono::Utc::now().timestamp();
        let tool_call_json = turn
            .tool_call
            .as_ref()
            .map(|c| serde_json::to_string(c).unwrap_or_default());
        let action_response_json = turn
            .action_response
            .as_ref()
            .map(|v| v.to_string());
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO turns (ts, user_text, tool_call_json, action_response_json, reply_text)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                ts,
                &turn.user_text,
                &tool_call_json,
                &action_response_json,
                &turn.reply_text,
            ],
        )
        .map_err(sql_err)?;
        Ok((conn.last_insert_rowid(), ts))
    }

    /// Return the most recent `n` turns, oldest-first. The caller
    /// gets `Turn` (not `StoredTurn`) because the recent slice
    /// feeds the same Ollama-context builder the in-memory path
    /// used pre-Phase 25 — keeping the shape identical avoids a
    /// flag-day in `bus_client.rs`.
    pub fn recent(&self, n: usize) -> Result<Vec<Turn>, LilithError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT user_text, tool_call_json, action_response_json, reply_text
                 FROM turns ORDER BY id DESC LIMIT ?1",
            )
            .map_err(sql_err)?;
        let n = n.min(i64::MAX as usize) as i64;
        let rows = stmt
            .query_map(params![n], |row| {
                let user_text: String = row.get(0)?;
                let tool_call_json: Option<String> = row.get(1)?;
                let action_response_json: Option<String> = row.get(2)?;
                let reply_text: String = row.get(3)?;
                Ok(Turn {
                    user_text,
                    tool_call: tool_call_json
                        .as_deref()
                        .and_then(|s| serde_json::from_str::<ToolCall>(s).ok()),
                    action_response: action_response_json
                        .as_deref()
                        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()),
                    reply_text,
                })
            })
            .map_err(sql_err)?;
        // ORDER BY id DESC LIMIT n gives us newest-first; reverse so
        // callers get oldest-first (matches the in-memory ring's
        // existing recent() shape).
        let mut out: Vec<Turn> = rows.filter_map(Result::ok).collect();
        out.reverse();
        Ok(out)
    }

    /// LIKE search over user_text + reply_text. Returns matching
    /// turns newest-first with their timestamp + rowid so the
    /// caller can surface "há 3 dias você perguntou …".
    ///
    /// V1 is plain LIKE (case-insensitive via lower()) — good
    /// enough for a few hundred turns. FTS5 lands when the table
    /// grows beyond the LIKE plan's comfort zone.
    pub fn search(&self, needle: &str, limit: usize) -> Result<Vec<StoredTurn>, LilithError> {
        let trimmed = needle.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let pattern = format!("%{}%", trimmed.to_lowercase());
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, ts, user_text, tool_call_json, action_response_json, reply_text
                 FROM turns
                 WHERE lower(user_text) LIKE ?1 OR lower(reply_text) LIKE ?1
                 ORDER BY id DESC
                 LIMIT ?2",
            )
            .map_err(sql_err)?;
        let limit = limit.min(i64::MAX as usize) as i64;
        let rows = stmt
            .query_map(params![pattern, limit], |row| {
                let id: i64 = row.get(0)?;
                let ts: i64 = row.get(1)?;
                let user_text: String = row.get(2)?;
                let tool_call_json: Option<String> = row.get(3)?;
                let action_response_json: Option<String> = row.get(4)?;
                let reply_text: String = row.get(5)?;
                Ok(StoredTurn {
                    id,
                    ts,
                    turn: Turn {
                        user_text,
                        tool_call: tool_call_json
                            .as_deref()
                            .and_then(|s| serde_json::from_str::<ToolCall>(s).ok()),
                        action_response: action_response_json
                            .as_deref()
                            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()),
                        reply_text,
                    },
                })
            })
            .map_err(sql_err)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    /// Delete turns older than `cutoff_ts` (UNIX epoch seconds).
    /// Returns the number of rows removed. Caller decides when to
    /// invoke — V1 callers won't, the store grows unbounded.
    /// Useful for a future "prune > 90 days" cron.
    pub fn trim_older_than(&self, cutoff_ts: i64) -> Result<usize, LilithError> {
        let conn = self.conn.lock().unwrap();
        let n = conn
            .execute("DELETE FROM turns WHERE ts < ?1", params![cutoff_ts])
            .map_err(sql_err)?;
        Ok(n)
    }

    #[cfg(test)]
    pub fn count(&self) -> Result<i64, LilithError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM turns", [], |row| row.get(0))
            .map_err(sql_err)
    }
}

fn sql_err(e: rusqlite::Error) -> LilithError {
    LilithError::Io(std::io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn turn(text: &str, reply: &str) -> Turn {
        Turn {
            user_text: text.into(),
            tool_call: None,
            action_response: None,
            reply_text: reply.into(),
        }
    }

    #[test]
    fn append_and_recent_roundtrips() {
        let store = TurnStore::in_memory().unwrap();
        store.append(&turn("oi lilith", "olá")).unwrap();
        store.append(&turn("abrir firefox", "abrindo")).unwrap();
        let r = store.recent(10).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].user_text, "oi lilith");
        assert_eq!(r[1].user_text, "abrir firefox");
    }

    #[test]
    fn recent_caps_to_n() {
        let store = TurnStore::in_memory().unwrap();
        for i in 0..10 {
            store
                .append(&turn(&format!("msg {i}"), &format!("reply {i}")))
                .unwrap();
        }
        let r = store.recent(3).unwrap();
        assert_eq!(r.len(), 3);
        // Newest 3 → 7, 8, 9 in oldest-first order.
        assert_eq!(r[0].user_text, "msg 7");
        assert_eq!(r[2].user_text, "msg 9");
    }

    #[test]
    fn search_matches_user_text() {
        let store = TurnStore::in_memory().unwrap();
        store.append(&turn("instala o gimp", "ok")).unwrap();
        store.append(&turn("abrir firefox", "abrindo")).unwrap();
        store.append(&turn("desinstala o gimp", "ok")).unwrap();
        let hits = store.search("gimp", 10).unwrap();
        assert_eq!(hits.len(), 2);
        // Newest-first.
        assert_eq!(hits[0].turn.user_text, "desinstala o gimp");
        assert_eq!(hits[1].turn.user_text, "instala o gimp");
    }

    #[test]
    fn search_matches_reply_text() {
        let store = TurnStore::in_memory().unwrap();
        store.append(&turn("oi", "instalei o Firefox às 3 da tarde")).unwrap();
        let hits = store.search("firefox", 10).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn search_is_case_insensitive() {
        let store = TurnStore::in_memory().unwrap();
        store.append(&turn("Abrir Firefox", "ok")).unwrap();
        let hits = store.search("firefox", 10).unwrap();
        assert_eq!(hits.len(), 1);
        let hits = store.search("FIREFOX", 10).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn empty_needle_returns_empty() {
        let store = TurnStore::in_memory().unwrap();
        store.append(&turn("oi", "ok")).unwrap();
        assert!(store.search("", 10).unwrap().is_empty());
        assert!(store.search("   ", 10).unwrap().is_empty());
    }

    #[test]
    fn tool_call_roundtrips() {
        let store = TurnStore::in_memory().unwrap();
        let t = Turn {
            user_text: "abrir firefox".into(),
            tool_call: Some(ToolCall {
                action: "app.open".into(),
                params: json!({ "app": "firefox" }),
            }),
            action_response: Some(json!({ "launched": true })),
            reply_text: "abrindo Firefox".into(),
        };
        store.append(&t).unwrap();
        let r = store.recent(1).unwrap();
        assert_eq!(r[0].tool_call.as_ref().unwrap().action, "app.open");
        assert_eq!(
            r[0].action_response
                .as_ref()
                .unwrap()
                .get("launched")
                .unwrap(),
            &json!(true)
        );
    }

    #[test]
    fn trim_older_than_removes_rows() {
        let store = TurnStore::in_memory().unwrap();
        let (_, ts) = store.append(&turn("old", "ok")).unwrap();
        store.append(&turn("new", "ok")).unwrap();
        // Cutoff after the first row's ts + 1 — removes the first.
        let removed = store.trim_older_than(ts + 1).unwrap();
        assert!(removed >= 1);
        assert!(store.count().unwrap() <= 1);
    }
}
