//! Jarvis Voice — V3 (STT + TTS).
//!
//! StartListening opens the default microphone (cpal, hosted in a
//! capture actor thread so the `!Send` stream doesn't infect the DBus
//! service). StopListening stops the stream, writes the captured
//! samples to a temporary WAV, runs whisper.cpp's `whisper-cli` against
//! it, and emits `TranscriptionFinal` with the recognised text.
//!
//! Speak synthesizes `text` via piper, writes a WAV, plays it through
//! `paplay`, and reports `spoken: true` when playback completes.
//!
//! See ADR 0009 for the scope split and `module.md` for the contract.

mod capture;
mod hotword;
mod stt;
mod tts;

use capture::CaptureHandle;
use hotword::HotwordHandle;
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
    hotword: HotwordHandle,
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
    /// Synthesizes via piper, plays via paplay. Returns immediately with
    /// `{ spoken: true }` once the speaking state is engaged; the actual
    /// playback runs in a background task that transitions back to idle
    /// when paplay exits. If piper or the player aren't available, the
    /// task emits a TranscriptionFailed signal (reused as a "voice
    /// pipeline error" channel) so the shell surfaces it.
    async fn speak(&self, text: &str, #[zbus(signal_context)] ctx: SignalContext<'_>) -> String {
        if text.trim().is_empty() {
            return json!({ "spoken": false, "reason": "empty text" }).to_string();
        }

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

        let text_owned = text.to_string();
        let ctx_owned = ctx.to_owned();
        let state_handle = self.state.clone();
        tokio::spawn(async move {
            if let Err(e) = tts::speak(&text_owned).await {
                tracing::warn!(error = %e, "TTS speak failed");
                if let Err(e2) =
                    VoiceService::transcription_failed(&ctx_owned, &e.to_string()).await
                {
                    tracing::warn!("TranscriptionFailed emit failed: {e2}");
                }
            }
            let mut s = state_handle.lock().await;
            *s = State::Idle;
            if let Err(e) = Self::state_changed(&ctx_owned, s.as_str()).await {
                tracing::warn!("StateChanged emit failed: {e}");
            }
        });

        json!({ "spoken": true }).to_string()
    }

    async fn get_state(&self) -> String {
        let state = self.state.lock().await;
        json!({ "state": state.as_str() }).to_string()
    }

    /// Engage continuous wake-word listening. Runs an independent
    /// cpal stream alongside whatever else the daemon is doing — on
    /// PipeWire (Fedora default) multiple capture clients share the
    /// source cleanly. See ADR 0015 for the design.
    async fn start_hotword(&self) -> String {
        match self.hotword.enable().await {
            Ok(()) => {
                tracing::info!("Hotword enabled");
                json!({ "enabled": true }).to_string()
            }
            Err(e) => {
                tracing::warn!(error = %e, "StartHotword failed");
                json!({ "enabled": false, "reason": e.to_string() }).to_string()
            }
        }
    }

    /// Disengage hotword listening. Idempotent.
    async fn stop_hotword(&self) -> String {
        self.hotword.disable().await;
        tracing::info!("Hotword disabled");
        json!({ "enabled": false }).to_string()
    }

    async fn get_hotword_enabled(&self) -> bool {
        self.hotword.is_enabled().await
    }

    #[zbus(signal)]
    async fn state_changed(ctx: &SignalContext<'_>, state: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn transcription_final(ctx: &SignalContext<'_>, text: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn transcription_failed(ctx: &SignalContext<'_>, reason: &str) -> zbus::Result<()>;

    /// Fires when the hotword actor matched a wake-word in its
    /// sliding-window transcript. `text` is the full transcript that
    /// matched — the shell strips the wake-word and feeds the
    /// remainder to Lilith. Empty payload means "no remainder"
    /// (the user said only the wake-word).
    #[zbus(signal)]
    async fn hotword_detected(ctx: &SignalContext<'_>, text: &str) -> zbus::Result<()>;
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

    tracing::info!("Starting Jarvis Voice (V4: STT + TTS + hotword)");

    let (hotword_handle, mut hotword_events) = hotword::spawn();

    let service = VoiceService {
        state: Arc::new(AsyncMutex::new(State::Idle)),
        capture: capture::spawn(),
        hotword: hotword_handle,
    };

    let conn = connection::Builder::session()?
        .name("com.jarvis.Voice")?
        .serve_at("/com/jarvis/Voice", service)?
        .build()
        .await?;

    // Bridge the hotword actor's event channel to the DBus signal.
    // SignalContext::new ties the signal emission to the connection
    // + the object path; the interface name comes from the #[zbus]
    // attribute on the signal declaration.
    let signal_ctx = SignalContext::new(&conn, "/com/jarvis/Voice")?;
    tokio::spawn(async move {
        while let Some(text) = hotword_events.recv().await {
            if let Err(e) = VoiceService::hotword_detected(&signal_ctx, &text).await {
                tracing::warn!("HotwordDetected emit failed: {e}");
            }
        }
    });

    tracing::info!("Voice ready on com.jarvis.Voice");

    // Auto-resume hotword if the user had it on last session. The
    // setting is owned by com.jarvis.Settings which may not be up
    // yet at the moment we ask — give it a few seconds of retries
    // before giving up. If we can't reach it, leave hotword off.
    // Self-call goes back through DBus rather than reaching into the
    // handle directly so the same StartHotword path runs and any
    // observers (shell's hotwordEnabled property) see the transition.
    let conn_for_resume = conn.clone();
    tokio::spawn(async move {
        for attempt in 0..20 {
            if let Some(true) = read_bool_setting(&conn_for_resume, "voice.hotword.enabled").await {
                tracing::info!("Restoring hotword from settings");
                let proxy = match zbus::Proxy::new(
                    &conn_for_resume,
                    "com.jarvis.Voice",
                    "/com/jarvis/Voice",
                    "com.jarvis.Voice",
                )
                .await
                {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!(error = %e, "hotword self-call proxy failed");
                        return;
                    }
                };
                if let Err(e) = proxy.call::<_, _, String>("StartHotword", &()).await {
                    tracing::warn!(error = %e, "hotword auto-enable failed");
                }
                return;
            }
            if attempt == 0 {
                tracing::info!("voice.hotword.enabled not set or false; staying off");
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    });

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}

/// Best-effort `Get` against the Settings daemon. Returns `None` when
/// the daemon is unreachable, the key is missing, or the value isn't a
/// bool — caller treats all three as "default false".
async fn read_bool_setting(conn: &zbus::Connection, key: &str) -> Option<bool> {
    let proxy = zbus::Proxy::new(
        conn,
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
    parsed.get("value")?.as_bool()
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
