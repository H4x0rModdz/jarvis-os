use crate::error::LilithError;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

/// Persistent key-value fact store backed by SQLite.
///
/// One row per user fact. Keys are matched case-insensitively on read so that
/// "Favorite Editor" and "favorite editor" resolve to the same row — the LLM
/// is inconsistent about casing and we'd rather match generously than miss.
///
/// Phase 2a — only this `facts` table. Phase 2b will add session turns
/// (`turns` table) so conversation context survives daemon restarts too.
pub struct FactStore {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fact {
    pub key: String,
    pub value: String,
    pub set_at: String,
    pub updated_at: String,
}

impl FactStore {
    pub fn open(path: &Path) -> Result<Self, LilithError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)
            .map_err(|e| LilithError::Io(std::io::Error::other(e.to_string())))?;
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
            "CREATE TABLE IF NOT EXISTS facts (
                key_normalized TEXT PRIMARY KEY,
                key_original   TEXT NOT NULL,
                value          TEXT NOT NULL,
                set_at         TEXT NOT NULL,
                updated_at     TEXT NOT NULL
            );",
        )
        .map_err(|e| LilithError::Io(std::io::Error::other(e.to_string())))?;
        Ok(())
    }

    pub fn remember(&self, key: &str, value: &str) -> Result<Fact, LilithError> {
        let now = chrono::Utc::now().to_rfc3339();
        let key_norm = normalize(key);
        let conn = self.conn.lock().unwrap();

        let existed: Option<String> = conn
            .query_row(
                "SELECT set_at FROM facts WHERE key_normalized = ?1",
                params![&key_norm],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_err)?;

        let set_at = existed.unwrap_or_else(|| now.clone());

        conn.execute(
            "INSERT INTO facts (key_normalized, key_original, value, set_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(key_normalized) DO UPDATE SET
               key_original = excluded.key_original,
               value        = excluded.value,
               updated_at   = excluded.updated_at",
            params![&key_norm, key, value, &set_at, &now],
        )
        .map_err(sql_err)?;

        Ok(Fact {
            key: key.to_string(),
            value: value.to_string(),
            set_at,
            updated_at: now,
        })
    }

    pub fn recall(&self, key: &str) -> Result<Option<Fact>, LilithError> {
        let key_norm = normalize(key);
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT key_original, value, set_at, updated_at
             FROM facts WHERE key_normalized = ?1",
            params![&key_norm],
            |row| {
                Ok(Fact {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    set_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(sql_err)
    }

    /// Returns true if a row was deleted.
    pub fn forget(&self, key: &str) -> Result<bool, LilithError> {
        let key_norm = normalize(key);
        let conn = self.conn.lock().unwrap();
        let n = conn
            .execute(
                "DELETE FROM facts WHERE key_normalized = ?1",
                params![&key_norm],
            )
            .map_err(sql_err)?;
        Ok(n > 0)
    }

    pub fn list(&self) -> Result<Vec<Fact>, LilithError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT key_original, value, set_at, updated_at
                 FROM facts ORDER BY key_normalized",
            )
            .map_err(sql_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Fact {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    set_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })
            .map_err(sql_err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(sql_err)
    }
}

fn normalize(key: &str) -> String {
    key.trim().to_lowercase()
}

fn sql_err(e: rusqlite::Error) -> LilithError {
    LilithError::Io(std::io::Error::other(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remember_then_recall() {
        let s = FactStore::in_memory().unwrap();
        s.remember("favorite editor", "vscode").unwrap();
        let f = s.recall("favorite editor").unwrap().unwrap();
        assert_eq!(f.value, "vscode");
    }

    #[test]
    fn recall_case_insensitive() {
        let s = FactStore::in_memory().unwrap();
        s.remember("Favorite Browser", "firefox").unwrap();
        // different casing must still resolve to the same row
        assert_eq!(
            s.recall("favorite browser").unwrap().unwrap().value,
            "firefox"
        );
        assert_eq!(
            s.recall("FAVORITE BROWSER").unwrap().unwrap().value,
            "firefox"
        );
    }

    #[test]
    fn remember_updates_value_preserves_set_at() {
        let s = FactStore::in_memory().unwrap();
        let first = s.remember("mood", "happy").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let second = s.remember("mood", "tired").unwrap();
        // set_at frozen on first write, updated_at moves forward
        assert_eq!(second.set_at, first.set_at);
        assert!(second.updated_at > first.updated_at);
        assert_eq!(s.recall("mood").unwrap().unwrap().value, "tired");
    }

    #[test]
    fn forget_removes_fact() {
        let s = FactStore::in_memory().unwrap();
        s.remember("test", "v").unwrap();
        assert!(s.forget("test").unwrap());
        assert!(s.recall("test").unwrap().is_none());
        // second forget is a no-op
        assert!(!s.forget("test").unwrap());
    }

    #[test]
    fn list_is_sorted_by_normalized_key() {
        let s = FactStore::in_memory().unwrap();
        s.remember("Zebra", "stripes").unwrap();
        s.remember("apple", "red").unwrap();
        s.remember("Mango", "yellow").unwrap();
        let l = s.list().unwrap();
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].key, "apple");
        assert_eq!(l[1].key, "Mango");
        assert_eq!(l[2].key, "Zebra");
    }

    #[test]
    fn recall_unknown_returns_none() {
        let s = FactStore::in_memory().unwrap();
        assert!(s.recall("never set").unwrap().is_none());
    }
}
