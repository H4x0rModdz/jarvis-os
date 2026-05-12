//! `jarvis-voice-ctl` — a one-binary remote for the voice daemon.
//!
//! Bound to a global hotkey by labwc (and any future Jarvis compositor).
//! Calls a single `com.jarvis.Voice` method per invocation and prints the
//! daemon's reply, so the keybind script can stay one line of XML.
//!
//! ```text
//! jarvis-voice-ctl toggle    # StartListening if idle, else StopListening
//! jarvis-voice-ctl start
//! jarvis-voice-ctl stop
//! jarvis-voice-ctl cancel
//! ```
//!
//! Anything other than these four verbs exits with a non-zero status
//! and a usage line — easy to spot in a log when a keybind misfires.

use anyhow::{anyhow, Context, Result};
use zbus::{Connection, Proxy};

const SERVICE: &str = "com.jarvis.Voice";
const PATH: &str = "/com/jarvis/Voice";
const IFACE: &str = "com.jarvis.Voice";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let verb = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("usage: jarvis-voice-ctl <toggle|start|stop|cancel>"))?;

    let conn = Connection::session().await.context("connect session bus")?;
    let proxy = Proxy::new(&conn, SERVICE, PATH, IFACE)
        .await
        .context("build Voice proxy")?;

    let method = match verb.as_str() {
        "toggle" => current_toggle_method(&proxy).await?,
        "start" => "StartListening",
        "stop" => "StopListening",
        "cancel" => "Cancel",
        other => {
            return Err(anyhow!(
                "unknown verb '{other}'; expected toggle|start|stop|cancel"
            ));
        }
    };

    let reply: String = proxy
        .call(method, &())
        .await
        .with_context(|| format!("call {method}"))?;
    println!("{reply}");
    Ok(())
}

/// Resolve "toggle" against the daemon's current state. We ask once,
/// then pick StartListening vs StopListening — same logic the mic
/// button in the shell runs. Doing it client-side keeps the daemon's
/// interface minimal (no `Toggle()` method to maintain).
async fn current_toggle_method(proxy: &Proxy<'_>) -> Result<&'static str> {
    let state_json: String = proxy.call("GetState", &()).await.context("GetState")?;
    // Cheap parse — the daemon's response shape is `{"state": "..."}`. We
    // don't want a serde_json dep here for one field.
    let listening = state_json.contains("\"listening\"");
    Ok(if listening {
        "StopListening"
    } else {
        "StartListening"
    })
}
