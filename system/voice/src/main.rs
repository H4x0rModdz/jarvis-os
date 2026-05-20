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
mod voiceprint;

use capture::AudioCapture;
use hotword::HotwordHandle;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use stt::Stt;
use tokio::sync::Mutex as AsyncMutex;
use voiceprint::VoiceprintStore;
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
    capture: Arc<dyn AudioCapture>,
    hotword: HotwordHandle,
    voiceprints: Arc<VoiceprintStore>,
    stt: Arc<dyn Stt>,
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
        let stt = self.stt.clone();
        let ctx_owned = ctx.to_owned();
        let state_handle = self.state.clone();

        tokio::spawn(async move {
            let outcome = run_stt(capture, stt.as_ref()).await;
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

    /// Capture `seconds` of audio from the default mic and store the
    /// resulting feature vector as `user`'s voiceprint. Blocks the
    /// state machine for the duration — caller should expect the
    /// `StateChanged` cycle (idle → listening → processing → idle).
    /// V1 features are temporal log-RMS — see `voiceprint.rs` for the
    /// honest scope.
    async fn enroll_voiceprint(
        &self,
        user: &str,
        seconds: u32,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> String {
        let seconds = seconds.clamp(1, 10) as u64;
        match self.capture_seconds(seconds, &ctx).await {
            Ok(samples) => {
                let features = voiceprint::extract_features(&samples);
                if features.is_empty() {
                    return json!({ "ok": false, "reason": "no audio captured" }).to_string();
                }
                if let Err(e) = self.voiceprints.enroll(user, &features) {
                    return json!({ "ok": false, "reason": e.to_string() }).to_string();
                }
                tracing::info!(user, seconds, frames = features.len(), "Voiceprint enrolled");
                json!({
                    "ok": true,
                    "user": user,
                    "frames": features.len(),
                })
                .to_string()
            }
            Err(e) => json!({ "ok": false, "reason": e.to_string() }).to_string(),
        }
    }

    /// Capture a short sample and compare against `user`'s enrolled
    /// voiceprint. Returns `{ ok: bool, score: f32 }` where `ok` is
    /// `score >= MATCH_THRESHOLD`. `score` is exposed so callers (PAM
    /// module, settings UI) can show calibration feedback.
    async fn verify_voiceprint(
        &self,
        user: &str,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> String {
        let stored = match self.voiceprints.fetch(user) {
            Ok(Some(v)) => v,
            Ok(None) => {
                return json!({
                    "ok": false,
                    "reason": format!("user '{user}' not enrolled"),
                })
                .to_string()
            }
            Err(e) => {
                return json!({ "ok": false, "reason": e.to_string() }).to_string()
            }
        };
        // 2 s is long enough for "oi lilith" plus a beat, short enough
        // that the user doesn't wait forever to be let in.
        let samples = match self.capture_seconds(2, &ctx).await {
            Ok(s) => s,
            Err(e) => return json!({ "ok": false, "reason": e.to_string() }).to_string(),
        };
        let probe = voiceprint::extract_features(&samples);
        let score = voiceprint::similarity(&stored, &probe);
        let ok = score >= voiceprint::MATCH_THRESHOLD;
        tracing::info!(user, score, ok, "Voiceprint verify");
        json!({
            "ok": ok,
            "score": score,
            "threshold": voiceprint::MATCH_THRESHOLD,
        })
        .to_string()
    }

    /// Enrolled users, oldest enrollment first. Shell uses this to
    /// know whether to show "Enrolled ✓" or "Not enrolled" badges.
    async fn list_enrolled(&self) -> String {
        match self.voiceprints.list() {
            Ok(users) => json!({ "users": users }).to_string(),
            Err(e) => json!({ "users": [], "error": e.to_string() }).to_string(),
        }
    }

    /// Remove a user's voiceprint. Returns whether anything was
    /// removed (idempotent).
    async fn delete_voiceprint(&self, user: &str) -> String {
        match self.voiceprints.delete(user) {
            Ok(was) => json!({ "deleted": was }).to_string(),
            Err(e) => json!({ "deleted": false, "error": e.to_string() }).to_string(),
        }
    }

    #[zbus(signal)]
    async fn state_changed(ctx: &SignalContext<'_>, state: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn transcription_final(ctx: &SignalContext<'_>, text: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn transcription_failed(ctx: &SignalContext<'_>, reason: &str) -> zbus::Result<()>;

    /// Fires after EnrollVoiceprint/VerifyVoiceprint finish capture
    /// so the shell can stop showing a progress indicator. `op` is
    /// `"enroll"` or `"verify"`; `outcome` is the verbose JSON the
    /// method returned. Letting the shell bind a signal saves it from
    /// having to await the long-running DBus call from the UI thread.
    #[zbus(signal)]
    async fn voiceprint_complete(
        ctx: &SignalContext<'_>,
        op: &str,
        outcome: &str,
    ) -> zbus::Result<()>;

    /// Fires when the hotword actor matched a wake-word in its
    /// sliding-window transcript. `text` is the full transcript that
    /// matched — the shell strips the wake-word and feeds the
    /// remainder to Lilith. Empty payload means "no remainder"
    /// (the user said only the wake-word).
    #[zbus(signal)]
    async fn hotword_detected(ctx: &SignalContext<'_>, text: &str) -> zbus::Result<()>;
}

impl VoiceService {
    /// Hold the state machine for `seconds`, capturing audio the whole
    /// time. Used by EnrollVoiceprint and VerifyVoiceprint — both
    /// want a fixed-duration capture without going through the
    /// start/stop split the push-to-talk path uses.
    async fn capture_seconds(
        &self,
        seconds: u64,
        ctx: &SignalContext<'_>,
    ) -> anyhow::Result<Vec<i16>> {
        // Acquire the state guard up front so we fail fast if another
        // operation is already in flight — keeps the cpal stream
        // single-owner.
        let mut state = self.state.lock().await;
        if *state != State::Idle {
            anyhow::bail!("voice daemon busy ({})", state.as_str());
        }
        *state = State::Listening;
        let _ = Self::state_changed(ctx, State::Listening.as_str()).await;
        drop(state);

        self.capture.start().await?;
        tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
        let samples = self.capture.stop().await?;

        let mut state = self.state.lock().await;
        *state = State::Idle;
        let _ = Self::state_changed(ctx, State::Idle.as_str()).await;
        Ok(samples)
    }
}

/// Bottom half of the STT pipeline: finish the capture, hand the
/// samples to `transcribe_samples`. Splitting capture-stop from the
/// rest lets tests exercise the WAV write + Stt call without a real
/// cpal stream.
async fn run_stt(capture: Arc<dyn AudioCapture>, stt: &dyn Stt) -> anyhow::Result<String> {
    let samples = capture.stop().await?;
    transcribe_samples(samples, stt).await
}

/// Write the captured samples to a temp WAV, transcribe via the Stt
/// impl, clean up. Empty samples short-circuit to an empty string —
/// the daemon turns that into a TranscriptionFailed("no speech
/// detected") for the caller.
async fn transcribe_samples(samples: Vec<i16>, stt: &dyn Stt) -> anyhow::Result<String> {
    if samples.is_empty() {
        return Ok(String::new());
    }

    let wav_path = wav_temp_path();
    let wav_path_owned = wav_path.clone();
    let samples_for_write = samples;
    tokio::task::spawn_blocking(move || write_wav(&wav_path_owned, &samples_for_write)).await??;

    let text = stt.transcribe(&wav_path).await?;

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

    let vp_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".jarvis/voiceprints.db");
    let voiceprints = Arc::new(
        VoiceprintStore::open(&vp_path)
            .map_err(|e| anyhow::anyhow!("voiceprint store: {e}"))?,
    );
    tracing::info!(db = %vp_path.display(), "Voiceprint store ready");

    let stt: Arc<dyn Stt> = Arc::new(stt::WhisperCli);
    let capture: Arc<dyn AudioCapture> = Arc::new(capture::spawn());
    let service = VoiceService {
        state: Arc::new(AsyncMutex::new(State::Idle)),
        capture,
        hotword: hotword_handle,
        voiceprints,
        stt,
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
    use async_trait::async_trait;
    use std::path::Path;
    use std::sync::Mutex as StdMutex;
    use tokio::sync::Mutex as AsyncMutex;

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

    // ── Stt trait + state-machine helpers ────────────────────────────

    /// Scripted Stt — returns the next reply each time `transcribe`
    /// is called; records the wav paths so tests can assert the
    /// pipeline wrote a real file before the call.
    struct MockStt {
        replies: StdMutex<Vec<anyhow::Result<String>>>,
        seen_paths: StdMutex<Vec<std::path::PathBuf>>,
    }

    impl MockStt {
        fn new(replies: Vec<anyhow::Result<String>>) -> Self {
            Self {
                replies: StdMutex::new(replies),
                seen_paths: StdMutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<std::path::PathBuf> {
            self.seen_paths.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Stt for MockStt {
        async fn transcribe(&self, wav_path: &Path) -> anyhow::Result<String> {
            self.seen_paths.lock().unwrap().push(wav_path.to_path_buf());
            let mut replies = self.replies.lock().unwrap();
            if replies.is_empty() {
                anyhow::bail!("MockStt: no scripted replies left");
            }
            replies.remove(0)
        }
    }

    #[tokio::test]
    async fn transcribe_samples_empty_returns_empty() {
        let stt = MockStt::new(vec![]);
        let out = transcribe_samples(Vec::new(), &stt).await.unwrap();
        assert_eq!(out, "");
        // Mock was never called — the empty-samples branch short-
        // circuits before the WAV write + Stt dispatch.
        assert!(stt.calls().is_empty());
    }

    #[tokio::test]
    async fn transcribe_samples_writes_wav_and_calls_stt() {
        let stt = MockStt::new(vec![Ok("oi lilith".into())]);
        // 16 kHz × 0.1 s of silence — enough for a real WAV header
        // plus a payload `write_wav` can flush.
        let samples: Vec<i16> = vec![0; 1600];
        let out = transcribe_samples(samples, &stt).await.unwrap();
        assert_eq!(out, "oi lilith");
        // The mock saw a path; it should be cleaned up by the
        // helper but its name is checkable while the cleanup is
        // best-effort.
        let paths = stt.calls();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].to_string_lossy().contains("jarvis-voice-"));
    }

    #[tokio::test]
    async fn transcribe_samples_propagates_stt_errors() {
        let stt = MockStt::new(vec![Err(anyhow::anyhow!("model not found"))]);
        let samples: Vec<i16> = vec![0; 1600];
        let err = transcribe_samples(samples, &stt).await.unwrap_err();
        assert!(err.to_string().contains("model not found"));
    }

    // ── AudioCapture trait coverage ──────────────────────────────────

    /// In-memory AudioCapture: holds a `live` flag + the samples that
    /// the next `stop` should return. Mirrors what CaptureHandle does
    /// over an mpsc channel but without any real cpal stream.
    struct MockCapture {
        live: AsyncMutex<bool>,
        samples_on_stop: StdMutex<Vec<i16>>,
        stop_calls: StdMutex<u32>,
    }

    impl MockCapture {
        fn with_samples(samples: Vec<i16>) -> Self {
            Self {
                live: AsyncMutex::new(false),
                samples_on_stop: StdMutex::new(samples),
                stop_calls: StdMutex::new(0),
            }
        }

        fn stop_call_count(&self) -> u32 {
            *self.stop_calls.lock().unwrap()
        }
    }

    #[async_trait]
    impl AudioCapture for MockCapture {
        async fn start(&self) -> anyhow::Result<()> {
            let mut live = self.live.lock().await;
            if *live {
                anyhow::bail!("already capturing");
            }
            *live = true;
            Ok(())
        }

        async fn stop(&self) -> anyhow::Result<Vec<i16>> {
            let mut live = self.live.lock().await;
            *self.stop_calls.lock().unwrap() += 1;
            if !*live {
                anyhow::bail!("not capturing");
            }
            *live = false;
            Ok(self.samples_on_stop.lock().unwrap().clone())
        }

        async fn cancel(&self) {
            *self.live.lock().await = false;
        }
    }

    #[tokio::test]
    async fn capture_start_then_stop_returns_scripted_samples() {
        let mock = MockCapture::with_samples(vec![10, 20, 30]);
        mock.start().await.unwrap();
        let samples = mock.stop().await.unwrap();
        assert_eq!(samples, vec![10, 20, 30]);
        assert_eq!(mock.stop_call_count(), 1);
    }

    #[tokio::test]
    async fn capture_double_start_errors() {
        let mock = MockCapture::with_samples(vec![]);
        mock.start().await.unwrap();
        let err = mock.start().await.unwrap_err();
        assert!(err.to_string().contains("already capturing"));
    }

    #[tokio::test]
    async fn capture_stop_without_start_errors() {
        let mock = MockCapture::with_samples(vec![]);
        let err = mock.stop().await.unwrap_err();
        assert!(err.to_string().contains("not capturing"));
    }

    #[tokio::test]
    async fn capture_cancel_clears_live_state() {
        let mock = MockCapture::with_samples(vec![1, 2, 3]);
        mock.start().await.unwrap();
        mock.cancel().await;
        // After cancel, stop should error because we're no longer live.
        let err = mock.stop().await.unwrap_err();
        assert!(err.to_string().contains("not capturing"));
    }

    #[tokio::test]
    async fn run_stt_routes_through_audio_capture_and_stt() {
        // Full pipeline through the trait objects: capture stops with
        // 0.1 s of zero samples → write_wav → MockStt returns scripted
        // text. Confirms the type signatures all line up.
        let capture: Arc<dyn AudioCapture> =
            Arc::new(MockCapture::with_samples(vec![0; 1600]));
        let stt = MockStt::new(vec![Ok("ouvi você".into())]);
        // Start the mock first so stop() succeeds.
        capture.start().await.unwrap();

        let text = run_stt(capture, &stt).await.unwrap();
        assert_eq!(text, "ouvi você");
    }

    #[tokio::test]
    async fn run_stt_returns_empty_when_no_samples() {
        let capture: Arc<dyn AudioCapture> =
            Arc::new(MockCapture::with_samples(vec![]));
        let stt = MockStt::new(vec![]);
        capture.start().await.unwrap();
        let text = run_stt(capture, &stt).await.unwrap();
        assert_eq!(text, "");
    }

    #[test]
    fn state_transitions_are_exhaustive() {
        // Round-trip every variant through the state → &str → match.
        for state in [
            State::Idle,
            State::Listening,
            State::Processing,
            State::Speaking,
        ] {
            let s = state.as_str();
            let parsed = match s {
                "idle" => State::Idle,
                "listening" => State::Listening,
                "processing" => State::Processing,
                "speaking" => State::Speaking,
                other => panic!("unknown state: {other}"),
            };
            assert_eq!(parsed, state);
        }
    }
}
