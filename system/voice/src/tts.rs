//! Text-to-speech via the piper subprocess.
//!
//! `speak(text)` writes `text` on piper's stdin, asks it to drop a WAV
//! at a known temp path, then plays that WAV through `paplay` (the
//! PulseAudio / PipeWire compatibility tool). Two short subprocesses
//! beats wiring an audio output path inside the daemon — piper's own
//! release tarballs ship the runtime piper-tts binary which is the
//! easiest cross-distro path today.
//!
//! Defaults can be overridden via env vars:
//!   JARVIS_VOICE_TTS_BINARY   path to the piper binary
//!   JARVIS_VOICE_TTS_MODEL    path to a piper-voices `*.onnx` file
//!   JARVIS_VOICE_TTS_PLAYER   playback binary (default `paplay`)

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub const DEFAULT_BINARY: &str = "/usr/bin/piper";
pub const DEFAULT_MODEL: &str = "/usr/share/piper-voices/en_US-amy-medium.onnx";
pub const DEFAULT_PLAYER: &str = "paplay";

/// Text-to-speech abstraction. The DBus `speak` method spawns a task
/// that holds an `Arc<dyn Tts>`; the production build wires
/// `PiperTts` while tests use the `MockTts` in `main.rs`.
///
/// Empty input is a no-op (returns Ok) — that contract is part of
/// the trait so callers don't have to guard.
#[async_trait]
pub trait Tts: Send + Sync {
    async fn speak(&self, text: &str) -> Result<()>;

    /// Synthesize `text` to a WAV file and return its path **without** playing
    /// it — so the caller can read the audio for lip-sync amplitude (ADR 0028)
    /// before playback. `Ok(None)` means "not supported by this impl"; the
    /// daemon then falls back to `speak()`. Empty text is also `Ok(None)`.
    ///
    /// Default returns `None` so test/mock impls don't have to implement it and
    /// keep their existing `speak()`-only behaviour.
    async fn synthesize(&self, _text: &str) -> Result<Option<PathBuf>> {
        Ok(None)
    }

    /// Play a WAV produced by `synthesize`. Blocks until playback completes.
    /// Default is a no-op (paired with the `None` default of `synthesize`).
    async fn play(&self, _wav: &Path) -> Result<()> {
        Ok(())
    }
}

/// Production TTS: shells out to piper for synthesis + paplay for
/// playback. Same behaviour the free `speak()` function had —
/// wrapping it in an `Arc<dyn Tts>` is the testability gain.
pub struct PiperTts;

#[async_trait]
impl Tts for PiperTts {
    async fn speak(&self, text: &str) -> Result<()> {
        speak(text).await
    }
    async fn synthesize(&self, text: &str) -> Result<Option<PathBuf>> {
        synthesize_wav(text).await
    }
    async fn play(&self, wav: &Path) -> Result<()> {
        play_wav(wav).await
    }
}

/// No-op TTS — silently succeeds on every call. Used by the
/// state-machine tests in `main.rs` that don't care about the
/// audible output, only the state transitions. Gated behind
/// `#[cfg(test)]` since production always wires `PiperTts`;
/// without the gate, `-D dead_code` flags it on the release
/// build (the build_service helper that constructs it is itself
/// test-only).
#[cfg(test)]
pub struct NoopTts;

#[cfg(test)]
#[async_trait]
impl Tts for NoopTts {
    async fn speak(&self, _text: &str) -> Result<()> {
        Ok(())
    }
}

/// Synthesize `text` and play it. Blocks until playback finishes — the
/// caller wraps this in `tokio::spawn` so the DBus method returns
/// quickly. Kept as a free function for the existing unit tests +
/// any consumer that doesn't need the trait indirection.
///
/// Composed from `synthesize_wav` + `play_wav` so the daemon's lip-sync path
/// can sit between the two (read the WAV, then play it); this fallback runs
/// when synthesis isn't supported / the caller doesn't need the envelope.
pub async fn speak(text: &str) -> Result<()> {
    let wav_path = match synthesize_wav(text).await? {
        Some(p) => p,
        None => return Ok(()), // empty text
    };
    let result = play_wav(&wav_path).await;
    // Best-effort cleanup whether playback succeeded or not.
    let _ = tokio::fs::remove_file(&wav_path).await;
    result
}

/// Run piper to synthesize `text` into a temp WAV, returning its path. Does
/// **not** play or delete it — the caller owns the file (plays it, optionally
/// reads it for the lip-sync envelope, then cleans up). `Ok(None)` for empty
/// text. See `Tts::synthesize`.
pub async fn synthesize_wav(text: &str) -> Result<Option<PathBuf>> {
    if text.trim().is_empty() {
        return Ok(None);
    }

    let binary = env_path("JARVIS_VOICE_TTS_BINARY", DEFAULT_BINARY);
    let model = env_path("JARVIS_VOICE_TTS_MODEL", DEFAULT_MODEL);

    if !binary.exists() {
        return Err(anyhow!(
            "piper binary missing at {} (set JARVIS_VOICE_TTS_BINARY to override)",
            binary.display()
        ));
    }
    if !model.exists() {
        return Err(anyhow!(
            "piper voice model missing at {} (set JARVIS_VOICE_TTS_MODEL to override)",
            model.display()
        ));
    }

    let wav_path = wav_temp_path();

    // piper reads text on stdin, writes WAV to the path given by
    // `--output_file`. `--quiet` suppresses progress noise.
    let mut child = Command::new(&binary)
        .arg("--model")
        .arg(&model)
        .arg("--output_file")
        .arg(&wav_path)
        .arg("--quiet")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn {}", binary.display()))?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("piper stdin not captured"))?;
        stdin
            .write_all(text.as_bytes())
            .await
            .context("write text to piper stdin")?;
        // Drop closes stdin so piper sees EOF and starts synthesizing.
    }

    let status = child.wait().await.context("wait on piper")?;
    if !status.success() {
        return Err(anyhow!("piper exited with {status}"));
    }
    if !wav_path.exists() {
        return Err(anyhow!(
            "piper produced no output at {}",
            wav_path.display()
        ));
    }
    Ok(Some(wav_path))
}

/// Play a WAV through the configured player (paplay by default). Blocks until
/// playback completes. Does not delete the file.
pub async fn play_wav(wav: &Path) -> Result<()> {
    let player = std::env::var("JARVIS_VOICE_TTS_PLAYER").unwrap_or_else(|_| DEFAULT_PLAYER.into());
    let play_status = Command::new(&player)
        .arg(wav)
        .status()
        .await
        .with_context(|| format!("spawn {player}"))?;
    if !play_status.success() {
        return Err(anyhow!("{player} exited with {play_status}"));
    }
    Ok(())
}

/// Read a 16-bit PCM WAV into samples + its sample rate. Used by the lip-sync
/// path to derive the amplitude envelope from piper's output. Float WAVs are
/// down-converted to i16 so callers have one sample type to reason about.
pub fn read_wav_samples(path: &Path) -> Result<(Vec<i16>, u32)> {
    let mut reader = hound::WavReader::open(path).with_context(|| format!("open {path:?}"))?;
    let spec = reader.spec();
    let samples: Vec<i16> = match spec.sample_format {
        hound::SampleFormat::Int => reader.samples::<i16>().filter_map(|s| s.ok()).collect(),
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .filter_map(|s| s.ok())
            .map(|f| (f.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect(),
    };
    Ok((samples, spec.sample_rate))
}

/// Per-frame RMS amplitude of PCM `samples`, normalized so the loudest frame is
/// ~1.0. One value per `frame_ms` window — this is the lip-sync envelope that
/// drives the avatar's mouth-open weight while she speaks (ADR 0028). Silence
/// (or empty/zero input) yields an empty/all-zero curve so the mouth stays shut.
///
/// Honest scope: this is amplitude-driven lip *flap*, not phoneme-accurate
/// viseme timing. It convincingly opens the mouth on loud syllables and closes
/// it on pauses; the phoneme path is the documented upgrade behind this seam.
pub fn amplitude_envelope(samples: &[i16], sample_rate: u32, frame_ms: u32) -> Vec<f32> {
    if samples.is_empty() || sample_rate == 0 || frame_ms == 0 {
        return Vec::new();
    }
    let frame = ((sample_rate as u64 * frame_ms as u64) / 1000).max(1) as usize;
    let mut rms: Vec<f32> = samples
        .chunks(frame)
        .map(|c| {
            let sum: f64 = c.iter().map(|&s| (s as f64) * (s as f64)).sum();
            ((sum / c.len() as f64).sqrt()) as f32
        })
        .collect();
    let max = rms.iter().cloned().fold(0.0f32, f32::max);
    if max > 0.0 {
        for v in &mut rms {
            *v = (*v / max).clamp(0.0, 1.0);
        }
    }
    rms
}

fn env_path(var: &str, default: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

fn wav_temp_path() -> PathBuf {
    let pid = std::process::id();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("jarvis-voice-tts-{pid}-{ts}.wav"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_text_is_noop() {
        // No piper invocation, no error.
        assert!(speak("   ").await.is_ok());
        assert!(speak("").await.is_ok());
    }

    #[test]
    fn envelope_silence_is_all_zero() {
        let env = amplitude_envelope(&[0; 16_000], 16_000, 50);
        assert!(!env.is_empty());
        assert!(env.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn envelope_empty_input_is_empty() {
        assert!(amplitude_envelope(&[], 16_000, 50).is_empty());
        assert!(amplitude_envelope(&[1, 2, 3], 0, 50).is_empty());
        assert!(amplitude_envelope(&[1, 2, 3], 16_000, 0).is_empty());
    }

    #[test]
    fn envelope_normalizes_loudest_frame_to_one() {
        // One 50 ms frame of silence, one of full-scale tone. After
        // normalization the loud frame is 1.0 and the quiet one near 0.
        let rate = 16_000u32;
        let frame = (rate as usize * 50) / 1000; // samples per 50 ms
        let mut samples = vec![0i16; frame]; // quiet frame
        samples.extend(std::iter::repeat(i16::MAX).take(frame)); // loud frame
        let env = amplitude_envelope(&samples, rate, 50);
        assert_eq!(env.len(), 2);
        assert!(env[0] < 0.01, "quiet frame should be ~0, got {}", env[0]);
        assert!(
            (env[1] - 1.0).abs() < 1e-3,
            "loud frame should be 1.0, got {}",
            env[1]
        );
    }

    #[tokio::test]
    async fn errors_when_binary_missing() {
        std::env::set_var("JARVIS_VOICE_TTS_BINARY", "/this/path/does/not/exist/piper");
        std::env::set_var("JARVIS_VOICE_TTS_MODEL", "/etc/passwd");
        let r = speak("hello").await;
        assert!(r.is_err());
        assert!(
            r.unwrap_err().to_string().contains("piper binary missing"),
            "wrong error variant"
        );
        std::env::remove_var("JARVIS_VOICE_TTS_BINARY");
        std::env::remove_var("JARVIS_VOICE_TTS_MODEL");
    }
}
