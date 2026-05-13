//! `jarvis-voiceprint-ctl` — CLI wrapper around `com.jarvis.Voice`'s
//! voiceprint surface. Useful for QA, support scripts, and headless
//! enrollment when the Settings UI isn't an option.
//!
//! ```text
//! jarvis-voiceprint-ctl enroll <user> [seconds]
//! jarvis-voiceprint-ctl verify <user>
//! jarvis-voiceprint-ctl list
//! jarvis-voiceprint-ctl delete <user>
//! ```
//!
//! Mirrors the shape of `jarvis-lock-ctl` and `jarvis-voice-ctl`.
//! Exit codes:
//!   0  the daemon returned `ok: true` (or `deleted: true` for delete)
//!   1  the daemon returned `ok: false` (mismatch / not enrolled / etc.)
//!   2  daemon unreachable / CLI usage error

use anyhow::{anyhow, Context, Result};
use std::process::ExitCode;
use zbus::{Connection, Proxy};

const SERVICE: &str = "com.jarvis.Voice";
const PATH: &str = "/com/jarvis/Voice";
const IFACE: &str = "com.jarvis.Voice";

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

async fn run() -> Result<ExitCode> {
    let mut args = std::env::args().skip(1);
    let verb = args
        .next()
        .ok_or_else(|| anyhow!("usage: jarvis-voiceprint-ctl <enroll|verify|list|delete> [user]"))?;

    let conn = Connection::session().await.context("session bus")?;
    let proxy = Proxy::new(&conn, SERVICE, PATH, IFACE).await.context("proxy")?;

    match verb.as_str() {
        "enroll" => {
            let user = args.next().ok_or_else(|| anyhow!("missing <user>"))?;
            // 3 s by default — same window we use for whisper enrollment;
            // optional second arg lets the operator override (e.g. 5).
            let seconds: u32 = args
                .next()
                .map(|s| s.parse::<u32>())
                .transpose()
                .context("seconds arg")?
                .unwrap_or(3);
            eprintln!("Capturing {seconds}s for user '{user}' — speak now…");
            let reply: String = proxy
                .call("EnrollVoiceprint", &(user.as_str(), seconds))
                .await
                .context("EnrollVoiceprint")?;
            println!("{reply}");
            Ok(exit_from_ok(&reply))
        }
        "verify" => {
            let user = args.next().ok_or_else(|| anyhow!("missing <user>"))?;
            eprintln!("Listening for verification of '{user}'…");
            let reply: String = proxy
                .call("VerifyVoiceprint", &(user.as_str(),))
                .await
                .context("VerifyVoiceprint")?;
            println!("{reply}");
            Ok(exit_from_ok(&reply))
        }
        "list" => {
            let reply: String = proxy
                .call("ListEnrolled", &())
                .await
                .context("ListEnrolled")?;
            println!("{reply}");
            Ok(ExitCode::from(0))
        }
        "delete" => {
            let user = args.next().ok_or_else(|| anyhow!("missing <user>"))?;
            let reply: String = proxy
                .call("DeleteVoiceprint", &(user.as_str(),))
                .await
                .context("DeleteVoiceprint")?;
            println!("{reply}");
            Ok(exit_from_deleted(&reply))
        }
        other => Err(anyhow!(
            "unknown verb '{other}'; expected enroll|verify|list|delete"
        )),
    }
}

fn exit_from_ok(reply: &str) -> ExitCode {
    match serde_json::from_str::<serde_json::Value>(reply) {
        Ok(v) if v.get("ok").and_then(|x| x.as_bool()) == Some(true) => ExitCode::from(0),
        Ok(_) => ExitCode::from(1),
        Err(_) => ExitCode::from(2),
    }
}

fn exit_from_deleted(reply: &str) -> ExitCode {
    match serde_json::from_str::<serde_json::Value>(reply) {
        Ok(v) if v.get("deleted").and_then(|x| x.as_bool()) == Some(true) => ExitCode::from(0),
        Ok(_) => ExitCode::from(1),
        Err(_) => ExitCode::from(2),
    }
}
