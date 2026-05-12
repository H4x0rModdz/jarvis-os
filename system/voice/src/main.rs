//! Jarvis Voice — V2 (real STT, push-to-talk).
//!
//! StartListening opens the default microphone (cpal, hosted in a
//! capture actor thread so the `!Send` stream doesn't infect the DBus
//! service). StopListening stops the stream, writes the captured
//! samples to a temporary WAV, runs whisper.cpp's `whisper-cli` against
//! it, and emits `TranscriptionFinal` with the recognised text. TTS
//! still returns Unavailable — that's V3.
//!
//! See ADR 0009 for the scope split and `module.md` for the contract.

mod capture;
mod stt;

use capture::CaptureHandle;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use zbus::{connection, interface, SignalContext};

/// 16 kHz mono — matches the resampler target in `capture.rs` and what
/// whisper-cli expects.
const WAV_SAMPLE_RATE: u32 = 16_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Listening,
    Processing,
    Speaking,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            State::Idle => "idle",
            State::Listening => "listening",
            State::Processing => "processing",
            State::Speaking => "speaking",
        }
    }
}

struct VoiceService {
    state: Arc<AsyncMutex<State>>,
    capture: CaptureHandle,
}

#[interface(name = "com.jarvis.Voice")]
impl VoiceService {
    /// Begin capturing from the default microphone.
    /// Returns immediately; transcription happens after `StopListening`.
    async fn start_listening(&self, #[zbus(signal_context)] ctx: SignalContext<'_>) -> String {
        let mut state = self.state.lock().await;
        if *state != State::Idle {
            return json!({
                "started": false,
                "reason": format!("busy ({})", state.as_str())
            })
            .to_string();
        }

        if let Err(e) = self.capture.start().await {
            tracing::warn!(error = %e, "CaptureHandle::start failed");
            return json!({
                "started": false,
                "reason": format!("audio capture failed: {e}")
            })
            .to_string();
        }

        *state = State::Listening;
        let new_state = *state;
        drop(state);

        if let Err(e) = Self::state_changed(&ctx, new_state.as_str()).await {
            tracing::warn!("StateChanged emit failed: {e}");
        }

        tracing::info!("StartListening");
        json!({ "started": true }).to_string()
    }

    /// Stop the in-flight recording and run STT.
    async fn stop_listening(&self, #[zbus(signal_context)] ctx: SignalContext<'_>) -> String {
        let mut state = self.state.lock().await;
        if *state != State::Listening {
            return json!({
                "stopped": false,
                "reason": format!("not listening ({})", state.as_str())
            })
            .to_string();
        }
        *state = State::Processing;
        let processing = *state;
        drop(state);

        if let Err(e) = Self::state_changed(&ctx, processing.as_str()).await {
            tracing::warn!("StateChanged emit failed: {e}");
        }

        let capture = self.capture.clone();
        let ctx_owned = ctx.to_owned();
        let state_handle = self.state.clone();

        tokio::spawn(async move {
            let outcome = run_stt(capture).await;
            match outcome {
                Ok(text) if !text.is_empty() => {
                    if let Err(e) = VoiceService::transcription_final(&ctx_owned, &text).await {
                        tracing::warn!("TranscriptionFinal emit failed: {e}");
                    }
                }
                Ok(_) => {
                    if let Err(e) =
                        VoiceService::transcription_failed(&ctx_owned, "no speech detected").await
                    {
                        tracing::warn!("TranscriptionFailed emit failed: {e}");
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "STT failed");
                    if let Err(e2) =
                        VoiceService::transcription_failed(&ctx_owned, &e.to_string()).await
                    {
                        tracing::warn!("TranscriptionFailed emit failed: {e2}");
                    }
                }
            }
            let mut s = state_handle.lock().await;
            *s = State::Idle;
            if let Err(e) = VoiceService::state_changed(&ctx_owned, s.as_str()).await {
                tracing::warn!("StateChanged emit failed: {e}");
            }
        });

        json!({ "stopped": true }).to_string()
    }

    /// Abort whatever is in flight.
    async fn cancel(&self, #[zbus(signal_context)] ctx: SignalContext<'_>) -> String {
        self.capture.cancel().await;
        let mut state = self.state.lock().await;
        let was = *state;
        *state = State::Idle;
        drop(state);

        if let Err(e) = Self::state_changed(&ctx, State::Idle.as_str()).await {
            tracing::warn!("StateChanged emit failed: {e}");
        }
        tracing::info!(previous = %was.as_str(), "Cancel");
        json!({ "cancelled": true, "previous": was.as_str() }).to_string()
    }

    /// Speak `text` through the default audio sink.
    ///
    /// V3 wires piper here. V1/V2 cycle through the `speaking` state
    /// briefly so the shell exercises its visual paths today.
    async fn speak(&self, _text: &str, #[zbus(signal_context)] ctx: SignalContext<'_>) -> String {
        let mut state = self.state.lock().await;
        if *state != State::Idle {
            return json!({
                "spoken": false,
                "reason": format!("busy ({})", state.as_str())
            })
            .to_string();
        }
        *state = State::Speaking;
        drop(state);
        if let Err(e) = Self::state_changed(&ctx, State::Speaking.as_str()).await {
            tracing::warn!("StateChanged emit failed: {e}");
        }

        let ctx_owned = ctx.to_owned();
        let state_handle = self.state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            let mut s = state_handle.lock().await;
            *s = State::Idle;
            if let Err(e) = Self::state_changed(&ctx_owned, s.as_str()).await {
                tracing::warn!("StateChanged emit failed: {e}");
            }
        });

        json!({
            "spoken": false,
            "reason": "TTS not yet implemented (V3)"
        })
        .to_string()
    }

    async fn get_state(&self) -> String {
        let state = self.state.lock().await;
        json!({ "state": state.as_str() }).to_string()
    }

    #[zbus(signal)]
    async fn state_changed(ctx: &SignalContext<'_>, state: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn transcription_final(ctx: &SignalContext<'_>, text: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn transcription_failed(ctx: &SignalContext<'_>, reason: &str) -> zbus::Result<()>;
}

/// Bottom half of the STT pipeline: finish the capture, write the WAV,
/// invoke whisper.
async fn run_stt(capture: CaptureHandle) -> anyhow::Result<String> {
    let samples = capture.stop().await?;
    if samples.is_empty() {
        return Ok(String::new());
    }

    let wav_path = wav_temp_path();
    let wav_path_owned = wav_path.clone();
    let samples_for_write = samples;
    tokio::task::spawn_blocking(move || write_wav(&wav_path_owned, &samples_for_write)).await??;

    let text = stt::transcribe(&wav_path).await?;

    // Best-effort cleanup.
    let _ = tokio::fs::remove_file(&wav_path).await;

    Ok(text)
}

fn wav_temp_path() -> PathBuf {
    let pid = std::process::id();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("jarvis-voice-{pid}-{ts}.wav"))
}

fn write_wav(path: &std::path::Path, samples: &[i16]) -> anyhow::Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: WAV_SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for s in samples {
        writer.write_sample(*s)?;
    }
    writer.finalize()?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jarvis_voice=info".parse()?),
        )
        .init();

    tracing::info!("Starting Jarvis Voice (V2: STT via whisper.cpp)");

    let service = VoiceService {
        state: Arc::new(AsyncMutex::new(State::Idle)),
        capture: capture::spawn(),
    };

    let _conn = connection::Builder::session()?
        .name("com.jarvis.Voice")?
        .serve_at("/com/jarvis/Voice", service)?
        .build()
        .await?;

    tracing::info!("Voice ready on com.jarvis.Voice");

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_str_round_trips() {
        assert_eq!(State::Idle.as_str(), "idle");
        assert_eq!(State::Listening.as_str(), "listening");
        assert_eq!(State::Processing.as_str(), "processing");
        assert_eq!(State::Speaking.as_str(), "speaking");
    }

    #[test]
    fn wav_temp_path_unique_per_call() {
        let a = wav_temp_path();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let b = wav_temp_path();
        assert_ne!(a, b);
    }
}
