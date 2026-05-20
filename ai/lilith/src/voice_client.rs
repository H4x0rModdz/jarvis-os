//! Thin DBus client for `com.jarvis.Voice.Speak`.
//!
//! Used by the proactive loop to make Lilith actually speak
//! critical-urgency nudges out loud. Kept narrow — single fn,
//! all-best-effort: connection failures and method errors log +
//! swallow so a voice-daemon outage never takes down the
//! proactive ticker.
//!
//! The Voice daemon's Speak returns immediately ("spoken: true")
//! and runs piper in a spawned task on its end. So our call here
//! also returns fast; we don't wait for the audible playback to
//! finish.

use zbus::{Connection, Proxy};

const VOICE_SERVICE: &str = "com.jarvis.Voice";
const VOICE_PATH: &str = "/com/jarvis/Voice";
const VOICE_IFACE: &str = "com.jarvis.Voice";

/// Tell the voice daemon to speak `text`. Returns `true` when the
/// call landed on the daemon (regardless of whether playback then
/// succeeds), `false` on any error.
pub async fn speak(text: &str) -> bool {
    if text.trim().is_empty() {
        return false;
    }
    let conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "voice_client: session bus unavailable");
            return false;
        }
    };
    let proxy = match Proxy::new(&conn, VOICE_SERVICE, VOICE_PATH, VOICE_IFACE).await {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "voice_client: proxy build failed");
            return false;
        }
    };
    // Speak returns a JSON string ("{\"spoken\": true}" or similar).
    // We don't parse it — landing-the-call success is enough; the
    // daemon's own spawned task handles the actual TTS.
    match proxy.call::<_, _, String>("Speak", &(text,)).await {
        Ok(_) => true,
        Err(e) => {
            tracing::warn!(error = %e, "voice_client: Speak call failed");
            false
        }
    }
}
