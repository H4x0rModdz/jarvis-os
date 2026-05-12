//! Jarvis Voice — V1 surface.
//!
//! Exposes `com.jarvis.Voice` over the session bus and runs the state
//! machine the shell renders against. STT and TTS implementations land
//! in V2 and V3 respectively; this build returns `Unavailable` for both
//! so callers see the right error envelope instead of silent no-ops.
//!
//! See ADR 0009 for the scope split and `module.md` for the contract.

use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use zbus::{connection, interface, SignalContext};

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
        *state = State::Listening;
        let new_state = *state;
        drop(state);

        if let Err(e) = Self::state_changed(&ctx, new_state.as_str()).await {
            tracing::warn!("StateChanged emit failed: {e}");
        }

        // V1 stops short of real capture — V2 hooks cpal here. The
        // state still moves so the shell renders the listening UI;
        // a real StopListening call will route through processing
        // and then emit TranscriptionFailed("not implemented").
        tracing::info!("StartListening (surface-only)");
        json!({ "started": true, "phase": "v1-surface-only" }).to_string()
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

        // V1: there's no captured audio to process. Emit a failure so
        // the shell can surface "STT not yet implemented" instead of
        // hanging.
        let ctx_owned = ctx.to_owned();
        let state_handle = self.state.clone();
        tokio::spawn(async move {
            if let Err(e) =
                Self::transcription_failed(&ctx_owned, "STT not yet implemented (V2)").await
            {
                tracing::warn!("TranscriptionFailed emit failed: {e}");
            }
            let mut s = state_handle.lock().await;
            *s = State::Idle;
            if let Err(e) = Self::state_changed(&ctx_owned, s.as_str()).await {
                tracing::warn!("StateChanged emit failed: {e}");
            }
        });

        json!({ "stopped": true }).to_string()
    }

    /// Abort whatever is in flight.
    async fn cancel(&self, #[zbus(signal_context)] ctx: SignalContext<'_>) -> String {
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
    /// V3 wires piper here. V1 still cycles through the `speaking` state
    /// (briefly) and emits the matching signals — that lets the shell
    /// exercise its idle/speaking visual paths today, so V3 ships behind
    /// a known-good surface instead of changing the bridge contract.
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

        // Schedule the return-to-idle so the shell's "speaking" UI flashes
        // briefly instead of never appearing. V3 replaces this with the
        // piper subprocess + paplay.
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

    /// Snapshot the current state. Mostly for debugging — subscribers
    /// should bind to the `StateChanged` signal instead of polling.
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jarvis_voice=info".parse()?),
        )
        .init();

    tracing::info!("Starting Jarvis Voice (V1 surface)");

    let service = VoiceService {
        state: Arc::new(AsyncMutex::new(State::Idle)),
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

    // The DBus-method state-machine tests would need a full bus + signal
    // harness; those land alongside V2 when the methods do something
    // observable beyond emitting signals. For now state_str_round_trips
    // is a sanity guard against rename drift.
}
