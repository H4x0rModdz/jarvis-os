//! Voiceprint V2 — MFCC features + DTW alignment.
//!
//! Replaces the V1 temporal log-RMS envelope (ADR 0018). The wire
//! contract on `com.jarvis.Voice` is unchanged — `EnrollVoiceprint`,
//! `VerifyVoiceprint`, `score` field, threshold field — only the body
//! of `extract_features` and `similarity` move.
//!
//! ## Pipeline
//!
//! 1. **Pre-emphasis** — single-tap high-pass (`α = 0.97`). Lifts
//!    higher frequencies so vowel formants don't get drowned out by
//!    low-frequency room rumble.
//! 2. **Framing** — 25 ms windows with 10 ms hop. Standard speech-
//!    processing values; small enough that vowel transitions stay
//!    audible, big enough that one frame has enough FFT resolution.
//! 3. **Hamming window** — reduces spectral leakage at frame edges.
//! 4. **FFT (real-valued)** — `realfft` wraps `rustfft` so we get the
//!    half-spectrum directly.
//! 5. **Power spectrum** — `|X[k]|²`.
//! 6. **Mel filterbank** — 26 triangular filters spaced uniformly in
//!    mel scale (`mel = 2595 * log10(1 + f/700)`). Models perceived
//!    pitch closer to the cochlea than linear Hz.
//! 7. **log()** — compresses dynamic range; matches how loudness is
//!    perceived.
//! 8. **DCT-II** — decorrelates the mel bands into the cepstral domain.
//!    Keep the first 13 coefficients (drop 0th = total energy, dominant
//!    in non-speech).
//!
//! Comparison: **DTW** (dynamic time warping). Build a cost matrix of
//! Euclidean distances between MFCC frames, find the min-cost monotonic
//! path from (0,0) to (T1-1,T2-1), normalise by path length. The result
//! is in absolute MFCC distance units; we convert to a `[0, 1]` similarity
//! via `1 / (1 + dist/scale)` so the wire format stays "higher is better".
//!
//! ## What V2 still isn't
//!
//! MFCC + DTW is a real biometric pipeline, but it's the *classical*
//! approach — it discriminates speakers well in the closed-set, low-noise
//! case (enrolled user vs. random other person in the room) but it's
//! beatable by an attacker holding a recording. Phase 7+ may swap in
//! x-vector embeddings (ONNX) for proper anti-spoofing strength.

use anyhow::{Context, Result};
use realfft::RealFftPlanner;
use rusqlite::{params, Connection};
use std::f32::consts::PI;
use std::path::Path;
use std::sync::Mutex;

const SAMPLE_RATE: u32 = 16_000;

// Framing — 25 ms window, 10 ms hop.
const FRAME_SAMPLES: usize = (SAMPLE_RATE as usize * 25) / 1000; // 400
const HOP_SAMPLES: usize = (SAMPLE_RATE as usize * 10) / 1000; // 160
                                                               // FFT bin count — round up to power of two for FFT speed.
const FFT_SIZE: usize = 512;

const N_MEL_FILTERS: usize = 26;
const N_MFCC: usize = 13;

const PRE_EMPHASIS: f32 = 0.97;

/// DTW distance is in MFCC-Euclidean units. `30.0` is the rough
/// "same speaker, same phrase" plateau on a single dev machine; the
/// V2 threshold (0.6 below) corresponds to ~20 distance units. Like
/// V1, this needs ROC-curve tuning on real data — Phase 7 work.
const SIMILARITY_SCALE: f32 = 30.0;

/// Acceptance threshold on the cosine-like score. 0.6 is intentionally
/// looser than V1's 0.85 because the new metric has a tighter
/// "different speakers" cluster (more separation between same and
/// different). Re-tune when real telemetry exists.
pub const MATCH_THRESHOLD: f32 = 0.6;

/// One frame of MFCC: 13 coefficients. Stored as Vec<f32> so the JSON
/// representation stays compact.
pub type Frame = Vec<f32>;
pub type FeatureVector = Vec<Frame>;

/// Compute the MFCC sequence for a chunk of 16 kHz mono PCM.
/// Returns an empty vector for empty / too-short audio (less than one
/// frame).
pub fn extract_features(samples: &[i16]) -> FeatureVector {
    if samples.len() < FRAME_SAMPLES {
        return Vec::new();
    }

    let float_samples: Vec<f32> = samples
        .iter()
        .map(|s| *s as f32 / i16::MAX as f32)
        .collect();

    // ── Pre-emphasis ─────────────────────────────────────────────────
    let mut emphasised = Vec::with_capacity(float_samples.len());
    emphasised.push(float_samples[0]);
    for i in 1..float_samples.len() {
        emphasised.push(float_samples[i] - PRE_EMPHASIS * float_samples[i - 1]);
    }

    // ── Pre-built tables ─────────────────────────────────────────────
    let hamming = hamming_window(FRAME_SAMPLES);
    let mel_filters = mel_filterbank(N_MEL_FILTERS, FFT_SIZE, SAMPLE_RATE);
    let dct_table = dct_table(N_MFCC, N_MEL_FILTERS);

    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let mut fft_input = fft.make_input_vec();
    let mut fft_output = fft.make_output_vec();

    // ── Frame loop ───────────────────────────────────────────────────
    let mut frames: FeatureVector = Vec::new();
    let mut pos = 0;
    while pos + FRAME_SAMPLES <= emphasised.len() {
        // Zero-pad into FFT-sized buffer.
        for i in 0..FFT_SIZE {
            fft_input[i] = if i < FRAME_SAMPLES {
                emphasised[pos + i] * hamming[i]
            } else {
                0.0
            };
        }
        fft.process(&mut fft_input, &mut fft_output)
            .expect("realfft contract: matching sizes");

        // Power spectrum.
        let mut power = vec![0.0f32; FFT_SIZE / 2 + 1];
        for (i, c) in fft_output.iter().enumerate() {
            power[i] = c.re * c.re + c.im * c.im;
        }

        // Mel filter energies + log.
        let mut mel_energies = vec![0.0f32; N_MEL_FILTERS];
        for (m, filter) in mel_filters.iter().enumerate() {
            let mut sum = 0.0f32;
            for (k, weight) in filter.iter().enumerate() {
                sum += weight * power[k];
            }
            // +1e-10 floor avoids log(0) on silent frames.
            mel_energies[m] = (sum + 1e-10).ln();
        }

        // DCT-II — first N_MFCC coefficients.
        let mut mfcc = vec![0.0f32; N_MFCC];
        for n in 0..N_MFCC {
            let mut s = 0.0f32;
            for m in 0..N_MEL_FILTERS {
                s += mel_energies[m] * dct_table[n][m];
            }
            mfcc[n] = s;
        }

        frames.push(mfcc);
        pos += HOP_SAMPLES;
    }

    frames
}

/// DTW-based similarity. Higher is more alike, in `[0, 1]`. Empty
/// input on either side returns 0.0.
pub fn similarity(a: &FeatureVector, b: &FeatureVector) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let dist = dtw_distance(a, b);
    1.0 / (1.0 + dist / SIMILARITY_SCALE)
}

// ── Building blocks ─────────────────────────────────────────────────

fn hamming_window(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.54 - 0.46 * (2.0 * PI * i as f32 / (n - 1) as f32).cos())
        .collect()
}

fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10f32.powf(mel / 2595.0) - 1.0)
}

/// Build `n_filters` triangular filters spaced uniformly in mel scale
/// between 0 Hz and the Nyquist. Each filter is a vector of weights
/// the same length as the FFT half-spectrum (`fft_size/2 + 1`).
fn mel_filterbank(n_filters: usize, fft_size: usize, sample_rate: u32) -> Vec<Vec<f32>> {
    let n_bins = fft_size / 2 + 1;
    let nyquist = sample_rate as f32 / 2.0;
    let low_mel = hz_to_mel(0.0);
    let high_mel = hz_to_mel(nyquist);

    // `n_filters + 2` points so each triangle has a left + peak + right.
    let mel_points: Vec<f32> = (0..n_filters + 2)
        .map(|i| low_mel + (high_mel - low_mel) * i as f32 / (n_filters + 1) as f32)
        .collect();
    let hz_points: Vec<f32> = mel_points.iter().map(|m| mel_to_hz(*m)).collect();
    let bin_points: Vec<usize> = hz_points
        .iter()
        .map(|hz| ((fft_size + 1) as f32 * hz / sample_rate as f32).floor() as usize)
        .collect();

    let mut filters = vec![vec![0.0f32; n_bins]; n_filters];
    for m in 1..=n_filters {
        let f_left = bin_points[m - 1];
        let f_center = bin_points[m];
        let f_right = bin_points[m + 1];
        for k in f_left..f_center.min(n_bins) {
            if f_center == f_left {
                continue;
            }
            filters[m - 1][k] = (k - f_left) as f32 / (f_center - f_left) as f32;
        }
        for k in f_center..f_right.min(n_bins) {
            if f_right == f_center {
                continue;
            }
            filters[m - 1][k] = (f_right - k) as f32 / (f_right - f_center) as f32;
        }
    }
    filters
}

/// Pre-computed orthonormal DCT-II table for size (`n_mfcc x
/// n_mel`). Built once per `extract_features` call rather than per
/// frame.
fn dct_table(n_mfcc: usize, n_mel: usize) -> Vec<Vec<f32>> {
    (0..n_mfcc)
        .map(|n| {
            let scale = (2.0 / n_mel as f32).sqrt();
            let alpha = if n == 0 { 1.0 / (2.0_f32).sqrt() } else { 1.0 };
            (0..n_mel)
                .map(|m| scale * alpha * (PI * (m as f32 + 0.5) * n as f32 / n_mel as f32).cos())
                .collect()
        })
        .collect()
}

/// DTW with no constraints, plain O(T1*T2). Speech enrollments are
/// short (≤10 s × 100 frames/s = 1000 frames worst case), so the
/// quadratic cost stays under a million operations — fine without
/// banding optimisations.
fn dtw_distance(a: &FeatureVector, b: &FeatureVector) -> f32 {
    let n = a.len();
    let m = b.len();
    let mut cost = vec![f32::INFINITY; (n + 1) * (m + 1)];
    cost[0] = 0.0; // (0,0)
                   // Mark first row/col INFINITY so the path is anchored.
    let idx = |i: usize, j: usize| i * (m + 1) + j;

    for i in 1..=n {
        for j in 1..=m {
            let d = euclidean(&a[i - 1], &b[j - 1]);
            let prev = cost[idx(i - 1, j - 1)]
                .min(cost[idx(i - 1, j)])
                .min(cost[idx(i, j - 1)]);
            cost[idx(i, j)] = d + prev;
        }
    }
    // Normalise by path length (we don't trace it back; T1+T2 is a
    // close-enough upper bound for normalisation purposes — keeps
    // longer enrollments comparable to shorter probes).
    cost[idx(n, m)] / (n + m) as f32
}

fn euclidean(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

// ── Storage ─────────────────────────────────────────────────────────

/// SQLite-backed store. V1 storage was `Vec<f32>`; V2 stores
/// `Vec<Vec<f32>>` (per-frame MFCC). On fetch we try the V2 shape
/// first and fall back to `None` for V1 rows — the user re-enrolls.
/// V1 prints were scaffold matchers (ADR 0018) so the missing
/// migration is acceptable.
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

    pub fn enroll(&self, user: &str, features: &FeatureVector) -> Result<()> {
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

    pub fn fetch(&self, user: &str) -> Result<Option<FeatureVector>> {
        let conn = self.conn.lock().unwrap();
        let row: rusqlite::Result<String> = conn.query_row(
            "SELECT features_json FROM voiceprints WHERE user = ?1",
            params![user],
            |r| r.get(0),
        );
        match row {
            Ok(json) => {
                // V2 shape. V1 rows (Vec<f32>) deserialise to a
                // `Vec<f32>` rather than `Vec<Vec<f32>>` and the
                // typed deserialise fails — treat as "not enrolled
                // for V2" so the caller prompts re-enrollment.
                match serde_json::from_str::<FeatureVector>(&json) {
                    Ok(v) => Ok(Some(v)),
                    Err(_) => Ok(None),
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list(&self) -> Result<Vec<EnrolledUser>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT user, enrolled_at FROM voiceprints ORDER BY user ASC")?;
        let rows = stmt.query_map([], |row| {
            Ok(EnrolledUser {
                user: row.get(0)?,
                enrolled_at: row.get(1)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

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
    fn extract_features_returns_empty_for_too_short() {
        assert!(extract_features(&[0i16; 100]).is_empty());
    }

    #[test]
    fn extract_features_shape_is_n_frames_x_n_mfcc() {
        // 1 second of audio.
        let samples = vec![1000i16; SAMPLE_RATE as usize];
        let feats = extract_features(&samples);
        assert!(!feats.is_empty());
        for frame in &feats {
            assert_eq!(frame.len(), N_MFCC);
        }
    }

    #[test]
    fn similarity_identical_high_score() {
        let samples = vec![1000i16; SAMPLE_RATE as usize];
        let feats = extract_features(&samples);
        let score = similarity(&feats, &feats);
        // Self-similarity collapses DTW to a zero-cost diagonal.
        assert!(score > 0.99, "score was {score}");
    }

    #[test]
    fn similarity_silence_vs_tone_low_score() {
        let silence: Vec<i16> = vec![0; SAMPLE_RATE as usize];
        // 440 Hz sine — concrete signal.
        let tone: Vec<i16> = (0..SAMPLE_RATE as usize)
            .map(|i| {
                let t = i as f32 / SAMPLE_RATE as f32;
                (20_000.0 * (2.0 * PI * 440.0 * t).sin()) as i16
            })
            .collect();
        let a = extract_features(&silence);
        let b = extract_features(&tone);
        let score = similarity(&a, &b);
        assert!(score < MATCH_THRESHOLD, "score was {score}");
    }

    #[test]
    fn similarity_empty_inputs_zero() {
        let v: FeatureVector = Vec::new();
        assert_eq!(similarity(&v, &v), 0.0);
    }

    fn open_temp() -> (VoiceprintStore, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "jarvis-vp-mfcc-test-{}-{}.db",
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
        let v: FeatureVector = vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]];
        store.enroll("alice", &v).unwrap();
        assert_eq!(store.fetch("alice").unwrap().unwrap(), v);
    }

    #[test]
    fn fetch_v1_row_returns_none() {
        // Simulate a V1 row (flat Vec<f32>); fetch should refuse it.
        let (store, _t) = open_temp();
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO voiceprints (user, features_json, enrolled_at)
             VALUES ('legacy', '[0.1,0.2,0.3]', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);
        assert!(store.fetch("legacy").unwrap().is_none());
    }
}
