//! Persistent notification history backed by SQLite.
//!
//! Replaces the RAM ring buffer from V2. Same surface: insert, delete
//! by id, clear all, snapshot the last N. SQLite's job: survive
//! daemon restarts so the drawer is useful even after a logout.
//!
//! Capacity is enforced at insert time — once the table hits
//! `MAX_ENTRIES`, the oldest row is dropped. Keeps the table bounded
//! without a background sweeper.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

/// Cap on stored rows. 500 is roughly a week of notifications for a
/// typical desktop — big enough to be useful, small enough that the
/// drawer's "load everything" path stays fast.
pub const MAX_ENTRIES: usize = 500;

#[derive(Debug, Clone)]
pub struct Entry {
    pub id: u32,
    pub app: String,
    pub summary: String,
    pub body: String,
    pub urgency: String,
    pub posted_at: String,
    /// Stored as a JSON array string so the schema doesn't grow a
    /// child table for what is usually empty.
    pub actions: Vec<String>,
}

/// Thread-safe handle. SQLite's `Connection` is `Send + !Sync`, so we
/// wrap it in a Mutex; every call serialises behind the lock. The
/// access pattern (a handful of writes per second at worst) doesn't
/// need anything fancier.
pub struct HistoryStore {
    conn: Mutex<Connection>,
}

impl HistoryStore {
    /// Open or create the database at `path`. Parent directories are
    /// created if absent.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS notifications (
                 id          INTEGER PRIMARY KEY,
                 app         TEXT NOT NULL,
                 summary     TEXT NOT NULL,
                 body        TEXT NOT NULL,
                 urgency     TEXT NOT NULL,
                 posted_at   TEXT NOT NULL,
                 actions_json TEXT NOT NULL DEFAULT '[]',
                 row_index   INTEGER NOT NULL  -- monotonic insertion order
             );
             CREATE INDEX IF NOT EXISTS idx_row_index ON notifications(row_index);",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Insert a new entry. Trims the oldest row when capacity is
    /// reached so the table never exceeds `MAX_ENTRIES`. Returns the
    /// id given by the caller (we don't generate ids — the daemon's
    /// `next_id` counter does).
    pub fn insert(&self, entry: &Entry) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // INSERT OR REPLACE so `replaces_id` semantics work — same id
        // overwrites in place.
        let row_index: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(row_index), 0) + 1 FROM notifications",
                [],
                |r| r.get(0),
            )
            .unwrap_or(1);

        let actions_json = serde_json::to_string(&entry.actions).unwrap_or_else(|_| "[]".into());
        conn.execute(
            "INSERT OR REPLACE INTO notifications
                (id, app, summary, body, urgency, posted_at, actions_json, row_index)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                entry.id,
                entry.app,
                entry.summary,
                entry.body,
                entry.urgency,
                entry.posted_at,
                actions_json,
                row_index,
            ],
        )?;

        // Trim oldest if over capacity.
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM notifications", [], |r| r.get(0))?;
        if count > MAX_ENTRIES as i64 {
            let drop_n = count - MAX_ENTRIES as i64;
            conn.execute(
                "DELETE FROM notifications WHERE id IN (
                    SELECT id FROM notifications ORDER BY row_index ASC LIMIT ?1
                 )",
                params![drop_n],
            )?;
        }
        Ok(())
    }

    /// Returns the last `limit` entries, oldest first (matches the V2
    /// `RecentNotifications` contract). `limit == 0` means "every row".
    pub fn recent(&self, limit: u32) -> Result<Vec<Entry>> {
        let conn = self.conn.lock().unwrap();
        let sql = if limit == 0 {
            "SELECT id, app, summary, body, urgency, posted_at, actions_json
             FROM notifications ORDER BY row_index ASC"
                .to_string()
        } else {
            // Pull the newest `limit`, then reverse on the Rust side
            // to deliver oldest-first like the caller expects.
            format!(
                "SELECT * FROM (
                    SELECT id, app, summary, body, urgency, posted_at, actions_json
                    FROM notifications ORDER BY row_index DESC LIMIT {limit}
                 ) ORDER BY row_index ASC"
            )
        };
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            let actions_json: String = row.get(6)?;
            let actions: Vec<String> = serde_json::from_str(&actions_json).unwrap_or_default();
            Ok(Entry {
                id: row.get(0)?,
                app: row.get(1)?,
                summary: row.get(2)?,
                body: row.get(3)?,
                urgency: row.get(4)?,
                posted_at: row.get(5)?,
                actions,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Drop one entry by id. Returns whether it was found.
    pub fn dismiss(&self, id: u32) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM notifications WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    /// Wipe every row; returns the count that was cleared.
    pub fn clear(&self) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM notifications", [], |r| r.get(0))?;
        conn.execute("DELETE FROM notifications", [])?;
        Ok(n as u32)
    }

    /// Highest id ever stored; the daemon uses this to seed its
    /// `next_id` counter on restart so we don't reuse ids that
    /// already appear in history.
    pub fn max_id(&self) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        let id: Option<i64> = conn
            .query_row("SELECT MAX(id) FROM notifications", [], |r| r.get(0))
            .optional()?
            .flatten();
        Ok(id.unwrap_or(0) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: u32, summary: &str) -> Entry {
        Entry {
            id,
            app: "test".into(),
            summary: summary.into(),
            body: "".into(),
            urgency: "normal".into(),
            posted_at: "2026-05-12T00:00:00Z".into(),
            actions: vec![],
        }
    }

    /// One-shot temp DB. Caller drops the path when the test ends —
    /// SQLite handles the file going away.
    fn open_temp() -> (HistoryStore, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "jarvis-notif-test-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        ));
        let _ = std::fs::remove_file(&path);
        let store = HistoryStore::open(&path).unwrap();
        (store, path)
    }

    #[test]
    fn insert_and_read_back() {
        let (store, _t) = open_temp();
        store.insert(&make_entry(1, "hello")).unwrap();
        let rows = store.recent(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].summary, "hello");
    }

    #[test]
    fn dismiss_drops_row() {
        let (store, _t) = open_temp();
        store.insert(&make_entry(1, "a")).unwrap();
        store.insert(&make_entry(2, "b")).unwrap();
        assert!(store.dismiss(1).unwrap());
        let rows = store.recent(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 2);
    }

    #[test]
    fn clear_empties_table() {
        let (store, _t) = open_temp();
        store.insert(&make_entry(1, "a")).unwrap();
        store.insert(&make_entry(2, "b")).unwrap();
        assert_eq!(store.clear().unwrap(), 2);
        assert_eq!(store.recent(10).unwrap().len(), 0);
    }

    #[test]
    fn capacity_evicts_oldest() {
        let (store, _t) = open_temp();
        // Stuff one over the cap.
        for i in 1..=(MAX_ENTRIES as u32 + 1) {
            store.insert(&make_entry(i, &format!("{i}"))).unwrap();
        }
        let rows = store.recent(0).unwrap();
        assert_eq!(rows.len(), MAX_ENTRIES);
        // The first one inserted (id=1) should be gone.
        assert!(rows.iter().all(|e| e.id != 1));
    }

    #[test]
    fn replace_by_id_updates_in_place() {
        let (store, _t) = open_temp();
        store.insert(&make_entry(7, "before")).unwrap();
        store.insert(&make_entry(7, "after")).unwrap();
        let rows = store.recent(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].summary, "after");
    }
}
