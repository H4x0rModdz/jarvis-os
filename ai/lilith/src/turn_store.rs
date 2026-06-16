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

/// A summary row — one chunk of old conversation compressed by the
/// LLM. The shell never sees these directly; they get injected into
/// the Ollama context as a single system note ("past conversation:
/// <text>") so the assistant retains long-term context after the
/// raw turns get pruned.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub id: i64,
    pub ts_from: i64,
    pub ts_to: i64,
    pub turn_count: i64,
    pub text: String,
}

impl TurnStore {
    pub fn open(path: &Path) -> Result<Self, LilithError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)
            .map_err(|e| LilithError::Io(std::io::Error::other(e.to_string())))?;
        crate::persistent::harden_db_permissions(path);
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
            CREATE INDEX IF NOT EXISTS idx_turns_ts ON turns(ts DESC);

            CREATE TABLE IF NOT EXISTS summaries (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                ts_from     INTEGER NOT NULL,
                ts_to       INTEGER NOT NULL,
                turn_count  INTEGER NOT NULL,
                text        TEXT    NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_summaries_ts ON summaries(ts_to DESC);",
        )
        .map_err(sql_err)?;

        // FTS5 virtual table + triggers. Best-effort — if the local
        // sqlite was built without FTS5 the CREATE statements fail
        // and we fall back to LIKE search in `search()`. Fedora
        // bootc has FTS5 enabled, so this is the production path.
        //
        // content='turns' + content_rowid='id' makes turns_fts an
        // "external content" FTS5 index: row data lives in `turns`,
        // FTS5 only stores the inverted index. The triggers below
        // keep them in sync on insert/update/delete.
        let _ = conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS turns_fts USING fts5(
                user_text,
                reply_text,
                content='turns',
                content_rowid='id',
                tokenize='unicode61 remove_diacritics 2'
            );
            CREATE TRIGGER IF NOT EXISTS turns_ai AFTER INSERT ON turns BEGIN
                INSERT INTO turns_fts(rowid, user_text, reply_text)
                VALUES (new.id, new.user_text, new.reply_text);
            END;
            CREATE TRIGGER IF NOT EXISTS turns_ad AFTER DELETE ON turns BEGIN
                INSERT INTO turns_fts(turns_fts, rowid, user_text, reply_text)
                VALUES ('delete', old.id, old.user_text, old.reply_text);
            END;
            CREATE TRIGGER IF NOT EXISTS turns_au AFTER UPDATE ON turns BEGIN
                INSERT INTO turns_fts(turns_fts, rowid, user_text, reply_text)
                VALUES ('delete', old.id, old.user_text, old.reply_text);
                INSERT INTO turns_fts(rowid, user_text, reply_text)
                VALUES (new.id, new.user_text, new.reply_text);
            END;",
        );

        // Backfill: if turns_fts exists but is empty (first migration
        // of an existing db), seed it from the live `turns` rows.
        // Counting against turns_fts is cheap — fts5 keeps a row
        // counter internally.
        if has_fts(conn) {
            let already_indexed: i64 = conn
                .query_row("SELECT count(*) FROM turns_fts", [], |row| row.get(0))
                .unwrap_or(0);
            if already_indexed == 0 {
                let _ = conn.execute_batch(
                    "INSERT INTO turns_fts(rowid, user_text, reply_text)
                     SELECT id, user_text, reply_text FROM turns;",
                );
            }
        }
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
        let action_response_json = turn.action_response.as_ref().map(|v| v.to_string());
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

    /// Search turn history. Tries FTS5 (BM25-ranked) first; falls
    /// back to LIKE if FTS5 isn't available or the user's query
    /// hits an FTS5 syntax edge (unbalanced quote, etc.). Returns
    /// matching turns newest-first.
    ///
    /// FTS5 ranking surfaces relevant turns over recency when both
    /// matter; for ties it falls back to the rowid (newest first).
    /// The Lilith memory.search caller doesn't pass complex queries
    /// — typical input is a single word or short phrase — so simple
    /// tokenisation is plenty.
    pub fn search(&self, needle: &str, limit: usize) -> Result<Vec<StoredTurn>, LilithError> {
        let trimmed = needle.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let limit_i64 = limit.min(i64::MAX as usize) as i64;
        let conn = self.conn.lock().unwrap();

        if has_fts(&conn) {
            // FTS5 path. Quote the needle to make it a literal phrase
            // — protects against `'` / `"` showing up in user input
            // and being misinterpreted as FTS5 operators.
            let phrase = fts5_quote(trimmed);
            let fts_result = conn
                .prepare(
                    "SELECT t.id, t.ts, t.user_text, t.tool_call_json,
                            t.action_response_json, t.reply_text
                     FROM turns_fts f
                     JOIN turns t ON t.id = f.rowid
                     WHERE turns_fts MATCH ?1
                     ORDER BY bm25(turns_fts), t.id DESC
                     LIMIT ?2",
                )
                .and_then(|mut stmt| {
                    let rows = stmt
                        .query_map(params![phrase, limit_i64], row_to_stored_turn)?
                        .filter_map(Result::ok)
                        .collect::<Vec<_>>();
                    Ok(rows)
                });
            if let Ok(rows) = fts_result {
                return Ok(rows);
            }
            // Fall through to LIKE on FTS5 query parse errors.
        }

        let pattern = format!("%{}%", trimmed.to_lowercase());
        let mut stmt = conn
            .prepare(
                "SELECT id, ts, user_text, tool_call_json, action_response_json, reply_text
                 FROM turns
                 WHERE lower(user_text) LIKE ?1 OR lower(reply_text) LIKE ?1
                 ORDER BY id DESC
                 LIMIT ?2",
            )
            .map_err(sql_err)?;
        let rows = stmt
            .query_map(params![pattern, limit_i64], row_to_stored_turn)
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

    /// Total live turns. Used by the auto-summary job to decide
    /// whether the store has grown enough to compress.
    pub fn count(&self) -> Result<i64, LilithError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM turns", [], |row| row.get(0))
            .map_err(sql_err)
    }

    /// Oldest `n` turns, id ASC. The summary job feeds these to
    /// Ollama and then deletes them by `id <= max_id`.
    pub fn oldest(&self, n: usize) -> Result<Vec<StoredTurn>, LilithError> {
        let conn = self.conn.lock().unwrap();
        let n = n.min(i64::MAX as usize) as i64;
        let mut stmt = conn
            .prepare(
                "SELECT id, ts, user_text, tool_call_json, action_response_json, reply_text
                 FROM turns ORDER BY id ASC LIMIT ?1",
            )
            .map_err(sql_err)?;
        let rows = stmt
            .query_map(params![n], row_to_stored_turn)
            .map_err(sql_err)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    /// Delete turns whose id is `<= max_id`. The summary job calls
    /// this after a successful Ollama round-trip so the same range
    /// can't be re-summarised on the next pass.
    pub fn delete_through(&self, max_id: i64) -> Result<usize, LilithError> {
        let conn = self.conn.lock().unwrap();
        let n = conn
            .execute("DELETE FROM turns WHERE id <= ?1", params![max_id])
            .map_err(sql_err)?;
        Ok(n)
    }

    /// Record a summary. The summary spans `ts_from..=ts_to` and
    /// represents `turn_count` original turns.
    pub fn record_summary(
        &self,
        ts_from: i64,
        ts_to: i64,
        turn_count: i64,
        text: &str,
    ) -> Result<i64, LilithError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO summaries (ts_from, ts_to, turn_count, text)
             VALUES (?1, ?2, ?3, ?4)",
            params![ts_from, ts_to, turn_count, text],
        )
        .map_err(sql_err)?;
        Ok(conn.last_insert_rowid())
    }

    /// Most-recent summary by ts_to, if any. Used by the context
    /// builder (#174) to prepend "past conversation" to the
    /// Ollama prompt.
    pub fn latest_summary(&self) -> Result<Option<Summary>, LilithError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, ts_from, ts_to, turn_count, text
                 FROM summaries ORDER BY ts_to DESC LIMIT 1",
            )
            .map_err(sql_err)?;
        let row = stmt
            .query_row([], |row| {
                Ok(Summary {
                    id: row.get(0)?,
                    ts_from: row.get(1)?,
                    ts_to: row.get(2)?,
                    turn_count: row.get(3)?,
                    text: row.get(4)?,
                })
            })
            .ok();
        Ok(row)
    }
}

fn sql_err(e: rusqlite::Error) -> LilithError {
    LilithError::Io(std::io::Error::other(e.to_string()))
}

/// Does `turns_fts` exist? Cheaper than checking sqlite's compile
/// options + handles the case where FTS5 was compiled in but our
/// init_schema CREATE VIRTUAL TABLE failed (unlikely but possible).
fn has_fts(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name='turns_fts'",
        [],
        |_| Ok(()),
    )
    .is_ok()
}

/// Wrap user input in FTS5 quotes so any `'`/`"`/operator chars get
/// treated as literal phrase text. Without this a query like
/// `it's broken` would parse as `it` AND `s` AND `broken` (or worse,
/// hit FTS5 quote-error).
fn fts5_quote(s: &str) -> String {
    // FTS5 phrase syntax: a double-quoted token where embedded `"` is
    // escaped by doubling. Wrap the whole string as one phrase.
    let escaped = s.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

/// Common row → StoredTurn mapper used by both FTS5 + LIKE paths.
fn row_to_stored_turn(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredTurn> {
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
        store
            .append(&turn("oi", "instalei o Firefox às 3 da tarde"))
            .unwrap();
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

    #[test]
    fn fts5_search_finds_phrase() {
        // Skips on builds without FTS5 — has_fts() reports correctly
        // even on in_memory dbs.
        let store = TurnStore::in_memory().unwrap();
        if !has_fts(&store.conn.lock().unwrap()) {
            return;
        }
        store
            .append(&turn("instala o gimp por favor", "ok"))
            .unwrap();
        store.append(&turn("desinstala o gimp", "feito")).unwrap();
        store.append(&turn("abrir firefox", "abrindo")).unwrap();

        let hits = store.search("gimp", 10).unwrap();
        assert_eq!(hits.len(), 2);
        // Both gimp matches present; FTS5 BM25 ordering is by
        // relevance, not by recency. Just check both made it.
        let texts: Vec<&str> = hits.iter().map(|h| h.turn.user_text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("instala o gimp")));
        assert!(texts.iter().any(|t| t.contains("desinstala o gimp")));
    }

    #[test]
    fn fts5_search_handles_quotes_in_input() {
        // Without fts5_quote(), the literal `'` would either error
        // out or split the phrase. With it, the lookup proceeds.
        let store = TurnStore::in_memory().unwrap();
        if !has_fts(&store.conn.lock().unwrap()) {
            return;
        }
        store.append(&turn("it's broken", "okay")).unwrap();
        // The single quote in the input must not crash the search.
        let hits = store.search("it's broken", 10).unwrap();
        assert!(!hits.is_empty());
    }

    #[test]
    fn fts5_diacritic_insensitive_matches() {
        // tokenize='unicode61 remove_diacritics 2' means café ≡ cafe.
        let store = TurnStore::in_memory().unwrap();
        if !has_fts(&store.conn.lock().unwrap()) {
            return;
        }
        store.append(&turn("vamos pro café", "claro")).unwrap();
        let hits = store.search("cafe", 10).unwrap();
        assert_eq!(hits.len(), 1);
    }
}
