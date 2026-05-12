//! `jarvis-lock-ctl` — one-shot CLI that triggers the lock daemon.
//!
//! Bound to Super+L by labwc; mirrors the `jarvis-voice-ctl` pattern.
//!
//! ```text
//! jarvis-lock-ctl lock          # engage the lock screen
//! ```
//!
//! Idempotent — calling while already locked is a no-op.

use anyhow::{anyhow, Context, Result};
use zbus::{Connection, Proxy};

const SERVICE: &str = "com.jarvis.Lock";
const PATH: &str = "/com/jarvis/Lock";
const IFACE: &str = "com.jarvis.Lock";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let verb = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("usage: jarvis-lock-ctl lock"))?;

    let conn = Connection::session().await.context("session bus")?;
    let proxy = Proxy::new(&conn, SERVICE, PATH, IFACE)
        .await
        .context("proxy")?;

    let method = match verb.as_str() {
        "lock" => "Lock",
        other => return Err(anyhow!("unknown verb '{other}'; expected lock")),
    };

    let reply: String = proxy.call(method, &()).await.context(method)?;
    println!("{reply}");
    Ok(())
}
