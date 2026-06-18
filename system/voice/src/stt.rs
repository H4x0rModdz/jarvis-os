//! Speech-to-text via the whisper.cpp `whisper-cli` subprocess.
//!
//! We hand whisper.cpp a 16-bit-PCM 16 kHz mono WAV (written from the
//! capture buffer) and ask it to drop a `.txt` next to the input.
//! Reading that file back keeps the API surface trivial — no need to
//! parse stdout or wire a streaming protocol for V2.
//!
//! Defaults can be overridden via env vars:
//!   JARVIS_VOICE_BINARY   path to the whisper-cli binary
//!   JARVIS_VOICE_MODEL    path to a ggml-*.bin model file
//!   JARVIS_VOICE_LANG     forced source language code, "auto" by default
//!
//! The `Stt` trait wraps `transcribe()` so tests can swap in a
//! scripted fake (see the test module in main.rs). Production wires
//! `WhisperCli` which calls the existing subprocess path.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Abstraction over speech-to-text so tests don't need a real
/// whisper-cli on PATH. Production impl is `WhisperCli`; tests
/// implement their own scripted versions.
#[async_trait]
pub trait Stt: Send + Sync {
    async fn transcribe(&self, wav_path: &Path) -> Result<String>;
}

/// Production STT: shells out to whisper-cli with the model + lang
/// resolution that `transcribe()` already does.
pub struct WhisperCli;

#[async_trait]
impl Stt for WhisperCli {
    async fn transcribe(&self, wav_path: &Path) -> Result<String> {
        transcribe(wav_path).await
    }
}

pub const DEFAULT_BINARY: &str = "/usr/bin/whisper-cli";
/// Baked, always-present model — the `small` multilingual ggml the image
/// ships (iso/Containerfile.builder). Used as the fallback when the
/// configured model isn't downloaded yet.
pub const DEFAULT_MODEL: &str = "/usr/share/whisper-models/ggml-small.bin";
/// System dir for image-baked models; user dir below for downloaded ones.
const SYSTEM_MODELS_DIR: &str = "/usr/share/whisper-models";
const DEFAULT_LANG: &str = "auto";

/// Where runtime-downloaded models land — `~/.local/share/whisper-models`.
pub fn user_models_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("whisper-models")
}

/// Download target for a model name, e.g. `medium` → the user dir's
/// `ggml-medium.bin`.
pub fn user_model_path(name: &str) -> PathBuf {
    user_models_dir().join(format!("ggml-{name}.bin"))
}

/// The whisper.cpp model repo URL for a model name.
pub fn download_url(name: &str) -> String {
    format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{name}.bin")
}

/// Resolve a `voice.model` value (a model NAME like `small`/`medium`, or a
/// full path) to an existing file on disk. Names are looked up in the user
/// download dir first, then the image's system dir. Returns `None` when the
/// model isn't present (e.g. selected in the panel but not downloaded yet).
pub fn resolve_model(name_or_path: &str) -> Option<PathBuf> {
    let v = name_or_path.trim();
    if v.is_empty() {
        return None;
    }
    if v.contains('/') {
        let p = PathBuf::from(v);
        return p.exists().then_some(p);
    }
    let file = format!("ggml-{v}.bin");
    let user = user_models_dir().join(&file);
    if user.exists() {
        return Some(user);
    }
    let sys = PathBuf::from(SYSTEM_MODELS_DIR).join(&file);
    sys.exists().then_some(sys)
}

/// Pick the model file for this transcription. Order: `voice.model` from
/// Settings (a name resolved to a present file) → `JARVIS_VOICE_MODEL` env
/// path → the baked default. Re-read every call so a panel change applies
/// without restarting the daemon. A configured-but-not-yet-downloaded model
/// falls back to the default so STT keeps working.
async fn resolve_model_setting() -> PathBuf {
    if let Some(val) = read_setting("voice.model").await {
        if let Some(p) = resolve_model(&val) {
            return p;
        }
        tracing::warn!(model = %val, "configured whisper model not present yet; using default");
        return PathBuf::from(DEFAULT_MODEL);
    }
    if let Some(p) = std::env::var_os("JARVIS_VOICE_MODEL") {
        return PathBuf::from(p);
    }
    PathBuf::from(DEFAULT_MODEL)
}

/// Run whisper-cli against `wav_path` and return the recognised text,
/// trimmed. Errors propagate the underlying message — Lilith / the shell
/// surface them verbatim to the user.
///
/// Resolution order for the source language:
///   1. `voice.language` from the Settings daemon (user toggle in
///      SettingsPanel).
///   2. `JARVIS_VOICE_LANG` env var (dev override).
///   3. `"auto"` — whisper guesses.
///
/// We re-read the setting on every call so a change in the panel takes
/// effect immediately, no daemon restart.
pub async fn transcribe(wav_path: &Path) -> Result<String> {
    let binary = env_path("JARVIS_VOICE_BINARY", DEFAULT_BINARY);
    let model = resolve_model_setting().await;
    let lang = read_setting("voice.language")
        .await
        .or_else(|| std::env::var("JARVIS_VOICE_LANG").ok())
        .unwrap_or_else(|| DEFAULT_LANG.into());

    if !binary.exists() {
        return Err(anyhow!(
            "whisper-cli binary missing at {} (set JARVIS_VOICE_BINARY to override)",
            binary.display()
        ));
    }
    if !model.exists() {
        return Err(anyhow!(
            "whisper model missing at {} (set JARVIS_VOICE_MODEL to override)",
            model.display()
        ));
    }

    // `-otxt -of <prefix>` writes "<prefix>.txt". Putting the prefix next
    // to the input keeps cleanup simple.
    let out_prefix = wav_path.with_extension("");
    let out_txt = out_prefix.with_extension("txt");

    let status = Command::new(&binary)
        .arg("-m")
        .arg(&model)
        .arg("-f")
        .arg(wav_path)
        .arg("-l")
        .arg(&lang)
        .arg("-otxt")
        .arg("-of")
        .arg(&out_prefix)
        .arg("-nt") // no timestamps
        .arg("-np") // no progress
        .status()
        .await
        .with_context(|| format!("spawn {}", binary.display()))?;

    if !status.success() {
        return Err(anyhow!(
            "whisper-cli exited with {} — check stderr in the daemon log",
            status
        ));
    }

    let text = tokio::fs::read_to_string(&out_txt)
        .await
        .with_context(|| format!("read whisper output {}", out_txt.display()))?;

    // Best-effort cleanup. Failing here doesn't change correctness.
    let _ = tokio::fs::remove_file(&out_txt).await;

    Ok(text.trim().to_string())
}

fn env_path(var: &str, default: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

/// Read a string from `com.jarvis.Settings`. Returns `None` when the
/// daemon is offline or the key isn't set — the caller chains through
/// env/default the same way they would with `std::env::var`.
async fn read_setting(key: &str) -> Option<String> {
    use zbus::{Connection, Proxy};
    let conn = Connection::session().await.ok()?;
    let proxy = Proxy::new(
        &conn,
        "com.jarvis.Settings",
        "/com/jarvis/Settings",
        "com.jarvis.Settings",
    )
    .await
    .ok()?;
    let response: String = proxy.call("Get", &(key,)).await.ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&response).ok()?;
    if !parsed.get("found")?.as_bool()? {
        return None;
    }
    parsed.get("value")?.as_str().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn env_path_uses_default_when_unset() {
        let unique = "JARVIS_VOICE_TEST_PATH_VAR_VEZASLPL";
        std::env::remove_var(unique);
        assert_eq!(
            env_path(unique, "/etc/passwd"),
            PathBuf::from("/etc/passwd")
        );
    }

    #[tokio::test]
    async fn transcribe_errors_when_binary_missing() {
        std::env::set_var(
            "JARVIS_VOICE_BINARY",
            "/this/path/does/not/exist/whisper-cli",
        );
        std::env::set_var("JARVIS_VOICE_MODEL", "/etc/passwd"); // exists, fails on whisper side
        let r = transcribe(Path::new("/tmp/does-not-matter.wav")).await;
        assert!(r.is_err());
        let msg = r.unwrap_err().to_string();
        assert!(
            msg.contains("whisper-cli binary missing"),
            "unexpected error: {msg}"
        );
        std::env::remove_var("JARVIS_VOICE_BINARY");
        std::env::remove_var("JARVIS_VOICE_MODEL");
    }
}
