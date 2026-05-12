//! SQLite-backed key-value store. One row per setting; values are
//! verbatim JSON strings (the daemon validates they parse, the schema
//! of the content is the caller's concern).
//!
//! Keys are case-sensitive — settings are operational state ("LILITH_MODEL"),
//! not user-narrative facts. Lilith's fact store normalizes; this one does
//! not. Mixing the two has bitten us before.

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
pub struct SettingEntry {
    pub key: String,
    pub value_json: String,
    pub updated_at: String,
}

pub struct SettingsStore {
    conn: Mutex<Connection>,
}

impl SettingsStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create settings parent dir {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("open settings db at {}", path.display()))?;
        // WAL keeps reads from blocking writes, NORMAL keeps writes durable
        // enough that a Set-then-Get round-trip always sees the new value.
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;",
        )?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS settings (
                 key        TEXT PRIMARY KEY,
                 value_json TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );",
        )?;
        Ok(())
    }

    /// Stores `value_json` for `key`, overwriting any existing entry.
    /// The caller is responsible for `value_json` being valid JSON —
    /// the daemon enforces that before this method runs.
    pub fn set(&self, key: &str, value_json: &str) -> Result<()> {
        if key.is_empty() {
            return Err(anyhow!("key must not be empty"));
        }
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
               value_json = excluded.value_json,
               updated_at = excluded.updated_at",
            params![key, value_json, now],
        )?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<SettingEntry>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT key, value_json, updated_at FROM settings WHERE key = ?1",
            params![key],
            |row| {
                Ok(SettingEntry {
                    key: row.get(0)?,
                    value_json: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    /// Returns true if a row was deleted.
    pub fn delete(&self, key: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(n > 0)
    }

    /// Returns key + updated_at for every entry, sorted by key. The
    /// value_json is intentionally NOT included — callers Get() what
    /// they need so a thousand-row List doesn't pull a thousand JSON
    /// blobs over DBus.
    pub fn list(&self) -> Result<Vec<SettingEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT key, value_json, updated_at FROM settings ORDER BY key")?;
        let rows = stmt.query_map([], |row| {
            Ok(SettingEntry {
                key: row.get(0)?,
                // Pull value_json out of the DB but the daemon-side
                // List method will strip it before serializing. Cheap
                // for now; matters at 10 000 settings, not 10.
                value_json: row.get(1)?,
                updated_at: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_then_get() {
        let s = SettingsStore::in_memory().unwrap();
        s.set("theme", "\"dark\"").unwrap();
        let e = s.get("theme").unwrap().unwrap();
        assert_eq!(e.key, "theme");
        assert_eq!(e.value_json, "\"dark\"");
    }

    #[test]
    fn get_missing_returns_none() {
        let s = SettingsStore::in_memory().unwrap();
        assert!(s.get("does-not-exist").unwrap().is_none());
    }

    #[test]
    fn set_overwrites() {
        let s = SettingsStore::in_memory().unwrap();
        s.set("model", "\"qwen3:4b\"").unwrap();
        s.set("model", "\"llama3:8b\"").unwrap();
        assert_eq!(s.get("model").unwrap().unwrap().value_json, "\"llama3:8b\"");
    }

    #[test]
    fn delete_reports_existence() {
        let s = SettingsStore::in_memory().unwrap();
        s.set("x", "true").unwrap();
        assert!(s.delete("x").unwrap());
        // Second delete on the same key is a no-op
        assert!(!s.delete("x").unwrap());
        // And the value really is gone
        assert!(s.get("x").unwrap().is_none());
    }

    #[test]
    fn list_returns_sorted_keys() {
        let s = SettingsStore::in_memory().unwrap();
        s.set("zoo", "1").unwrap();
        s.set("apple", "2").unwrap();
        s.set("mango", "3").unwrap();
        let l = s.list().unwrap();
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].key, "apple");
        assert_eq!(l[1].key, "mango");
        assert_eq!(l[2].key, "zoo");
    }

    #[test]
    fn keys_are_case_sensitive() {
        // Settings are operational identifiers like LILITH_MODEL; the
        // case-insensitive matching Lilith's facts use would cause two
        // identifiers that differ only in case to collide silently.
        let s = SettingsStore::in_memory().unwrap();
        s.set("Theme", "\"dark\"").unwrap();
        s.set("theme", "\"light\"").unwrap();
        assert_eq!(s.get("Theme").unwrap().unwrap().value_json, "\"dark\"");
        assert_eq!(s.get("theme").unwrap().unwrap().value_json, "\"light\"");
    }

    #[test]
    fn empty_key_rejected() {
        let s = SettingsStore::in_memory().unwrap();
        assert!(s.set("", "1").is_err());
    }
}
