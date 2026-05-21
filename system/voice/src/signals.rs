//! `VoiceSignalSink` — abstraction over the DBus signal emissions the
//! voice daemon makes during its DBus method implementations. Lets
//! tests drive the state machine (start/stop/cancel, speak,
//! voiceprint enroll/verify) without a live `SignalContext`.
//!
//! Two implementations:
//!
//! - `DbusVoiceSink` (production): wraps an owned `SignalContext` and
//!   forwards each call to the matching `com.jarvis.Voice` signal
//!   declared on `VoiceService`.
//!
//! - `NoopVoiceSink` (#[cfg(test)] / batch use): swallows every
//!   emission. A `RecordingVoiceSink` lives in the main.rs test
//!   module and captures `(name, args)` tuples so tests can assert
//!   on emission sequence.
//!
//! Same shape as the Lilith `SignalSink` from Phase 13 — kept
//! separate because the surfaces don't share signal names + the two
//! daemons evolve independently.

use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait VoiceSignalSink: Send + Sync {
    /// Fires every time the state machine moves between
    /// `idle | listening | processing | speaking`.
    async fn state_changed(&self, state: &str);

    /// Fires once per successful STT cycle with the recognised text.
    async fn transcription_final(&self, text: &str);

    /// Fires when an STT cycle errors out (no audio, model missing,
    /// piper failure on the TTS side, etc.). The voice daemon
    /// historically piggybacks TTS errors onto this signal too — the
    /// shell treats both as a single "voice pipeline error" channel.
    async fn transcription_failed(&self, reason: &str);
}

/// No-op convenience — useful for early init or anywhere a caller
/// doesn't have a real signal context handy. Currently unused by
/// the daemon itself (production uses `DbusVoiceSink`, tests build
/// a custom `RecordingVoiceSink`); kept as part of the trait's
/// public surface for downstream consumers.
#[allow(dead_code)]
pub struct NoopVoiceSink;

#[async_trait]
impl VoiceSignalSink for NoopVoiceSink {
    async fn state_changed(&self, _state: &str) {}
    async fn transcription_final(&self, _text: &str) {}
    async fn transcription_failed(&self, _reason: &str) {}
}

/// Helper for callers that don't need real signals.
#[allow(dead_code)]
pub fn noop_sink() -> Arc<dyn VoiceSignalSink> {
    Arc::new(NoopVoiceSink)
}
