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
use std::path::PathBuf;
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
pub async fn speak(text: &str) -> Result<()> {
    if text.trim().is_empty() {
        return Ok(());
    }

    let binary = env_path("JARVIS_VOICE_TTS_BINARY", DEFAULT_BINARY);
    let model = env_path("JARVIS_VOICE_TTS_MODEL", DEFAULT_MODEL);
    let player = std::env::var("JARVIS_VOICE_TTS_PLAYER").unwrap_or_else(|_| DEFAULT_PLAYER.into());

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

    let play_status = Command::new(&player)
        .arg(&wav_path)
        .status()
        .await
        .with_context(|| format!("spawn {player}"))?;

    // Best-effort cleanup whether playback succeeded or not.
    let _ = tokio::fs::remove_file(&wav_path).await;

    if !play_status.success() {
        return Err(anyhow!("{player} exited with {play_status}"));
    }
    Ok(())
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
