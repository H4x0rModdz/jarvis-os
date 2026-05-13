//! Voiceprint V2 — naive enrollment + matcher.
//!
//! Scope: ship the end-to-end round-trip (enroll → store → verify) so
//! the rest of the stack can be wired up (pam-jarvis V2, settings UI,
//! Lilith tool) before Phase 6 lands a real biometric matcher.
//!
//! ## What V1 does
//!
//! 1. Capture N seconds of mono 16 kHz audio (the daemon's existing
//!    capture path is reused).
//! 2. Slice it into 100 ms windows.
//! 3. For each window, compute log-RMS (compresses dynamic range so
//!    silence pauses don't dominate).
//! 4. Store the resulting `Vec<f32>` keyed by user.
//! 5. To verify, capture a shorter sample, compute its log-RMS curve,
//!    align lengths (truncate the longer), compute cosine similarity.
//! 6. Threshold at `MATCH_THRESHOLD` (0.85) — chosen by eyeballing two
//!    same-speaker samples vs. cross-speaker samples on a single dev
//!    machine. Not statistically validated; explicit pre-production
//!    knob in `module.md`.
//!
//! ## What V1 is NOT
//!
//! This is **temporal envelope similarity**, not a real voiceprint.
//! Two recordings of the same phrase by the same speaker score high;
//! a different speaker saying the same phrase with similar pacing also
//! scores high. Reliable biometric identity needs spectral features
//! (MFCC, x-vectors, etc.) — Phase 6 work. ADR 0016 documents the
//! eventual replacement.

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

/// 16 kHz mono — matches the capture pipeline.
const SAMPLE_RATE: u32 = 16_000;
/// Window size used to compute one log-RMS value.
const WINDOW_MS: usize = 100;
/// Cosine similarity threshold above which we accept a match. Tuned
/// loose for V1 — a future ROC-curve study moves this once real
/// biometric features land.
pub const MATCH_THRESHOLD: f32 = 0.85;

/// Compute the per-window log-RMS curve. Returns a `Vec<f32>` where
/// each entry covers `WINDOW_MS` of audio.
pub fn extract_features(samples: &[i16]) -> Vec<f32> {
    let window = (SAMPLE_RATE as usize * WINDOW_MS) / 1000;
    if window == 0 || samples.is_empty() {
        return Vec::new();
    }
    samples
        .chunks(window)
        .map(|chunk| {
            if chunk.is_empty() {
                return 0.0;
            }
            let sum_sq: f64 = chunk.iter().map(|s| (*s as f64).powi(2)).sum();
            let rms = (sum_sq / chunk.len() as f64).sqrt();
            (1.0 + rms).log10() as f32
        })
        .collect()
}

/// Cosine similarity over two equal-length vectors. Returns 0.0 for
/// degenerate inputs (empty or all-zero). When the two vectors differ
/// in length we truncate to the shorter — the caller decides whether
/// that's acceptable, but in practice enrollment captures are longer
/// than verification captures so the verification length wins.
pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let (a, b) = (&a[..n], &b[..n]);
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na < f32::EPSILON || nb < f32::EPSILON {
        return 0.0;
    }
    dot / (na * nb)
}

/// SQLite-backed store. Schema is one row per enrolled user.
pub struct VoiceprintStore {
    conn: Mutex<Connection>,
}

impl VoiceprintStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        let conn = Connection::open(path).with_context(|| format!("open {}", path.display()))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS voiceprints (
                 user TEXT PRIMARY KEY,
                 features_json TEXT NOT NULL,
                 enrolled_at TEXT NOT NULL
             );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Insert or replace a feature vector for `user`.
    pub fn enroll(&self, user: &str, features: &[f32]) -> Result<()> {
        let features_json = serde_json::to_string(features)?;
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO voiceprints (user, features_json, enrolled_at)
             VALUES (?1, ?2, ?3)",
            params![user, features_json, now],
        )?;
        Ok(())
    }

    /// Fetch the enrolled feature vector for `user`. `None` when the
    /// user isn't enrolled.
    pub fn fetch(&self, user: &str) -> Result<Option<Vec<f32>>> {
        let conn = self.conn.lock().unwrap();
        let row: rusqlite::Result<String> = conn.query_row(
            "SELECT features_json FROM voiceprints WHERE user = ?1",
            params![user],
            |r| r.get(0),
        );
        match row {
            Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Enrolled users; returned in lexicographic order so the UI has a
    /// predictable list.
    pub fn list(&self) -> Result<Vec<EnrolledUser>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT user, enrolled_at FROM voiceprints ORDER BY user ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(EnrolledUser {
                user: row.get(0)?,
                enrolled_at: row.get(1)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Delete a user's voiceprint. Idempotent.
    pub fn delete(&self, user: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let n = conn.execute("DELETE FROM voiceprints WHERE user = ?1", params![user])?;
        Ok(n > 0)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EnrolledUser {
    pub user: String,
    pub enrolled_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_features_empty_audio_empty_vector() {
        assert!(extract_features(&[]).is_empty());
    }

    #[test]
    fn extract_features_silence_is_low() {
        let samples = vec![0i16; SAMPLE_RATE as usize]; // 1 second silence
        let feats = extract_features(&samples);
        assert!(feats.iter().all(|x| *x < 0.5));
    }

    #[test]
    fn extract_features_loud_is_high() {
        let samples = vec![20_000i16; SAMPLE_RATE as usize];
        let feats = extract_features(&samples);
        assert!(feats.iter().all(|x| *x > 4.0));
    }

    #[test]
    fn similarity_identical_is_one() {
        let v = vec![1.0, 2.0, 3.0, 4.0];
        let s = similarity(&v, &v);
        assert!((s - 1.0).abs() < 1e-5);
    }

    #[test]
    fn similarity_opposite_is_minus_one() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        let s = similarity(&a, &b);
        assert!((s + 1.0).abs() < 1e-5);
    }

    #[test]
    fn similarity_zero_for_orthogonal() {
        let s = similarity(&[1.0, 0.0], &[0.0, 1.0]);
        assert!(s.abs() < 1e-5);
    }

    fn open_temp() -> (VoiceprintStore, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "jarvis-vp-test-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        ));
        let _ = std::fs::remove_file(&path);
        let store = VoiceprintStore::open(&path).unwrap();
        (store, path)
    }

    #[test]
    fn enroll_then_fetch_roundtrips() {
        let (store, _t) = open_temp();
        let v = vec![0.1, 0.2, 0.3];
        store.enroll("alice", &v).unwrap();
        let fetched = store.fetch("alice").unwrap().unwrap();
        assert_eq!(fetched, v);
    }

    #[test]
    fn fetch_unknown_user_is_none() {
        let (store, _t) = open_temp();
        assert!(store.fetch("nobody").unwrap().is_none());
    }

    #[test]
    fn enroll_replaces_existing() {
        let (store, _t) = open_temp();
        store.enroll("alice", &[1.0]).unwrap();
        store.enroll("alice", &[2.0]).unwrap();
        assert_eq!(store.fetch("alice").unwrap().unwrap(), vec![2.0]);
    }
}
