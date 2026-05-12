//! Jarvis Compat — the Windows compatibility daemon.
//!
//! V1: one method (`RunExe`), one shared Wine prefix at
//! `~/.jarvis/wine/default/`. Spawned children inherit the user's
//! Wayland/X11 environment so their windows land in the labwc
//! session. See ADR 0013 for what V2 adds.

use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use zbus::{connection, interface, SignalContext};

const WINE_BINARY: &str = "wine";
const WINE_ARCH: &str = "win64";

struct CompatService {
    /// Held during `wineboot --init` so concurrent first-time calls
    /// queue up behind the prefix bring-up instead of racing it.
    prefix_init: Arc<AsyncMutex<()>>,
}

#[interface(name = "com.jarvis.Compat")]
impl CompatService {
    /// Run a Windows binary under Wine.
    ///
    /// `path` is the absolute (or PATH-relative) location of the
    /// `.exe`. `args` is the argv tail passed verbatim. Returns
    /// `{ started, pid? }` immediately — the spawned process runs
    /// asynchronously. The `ProcessExited` signal fires when it
    /// terminates.
    async fn run_exe(
        &self,
        path: &str,
        args: Vec<String>,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> String {
        let exe = std::path::Path::new(path);
        if !exe.exists() {
            return json!({
                "started": false,
                "reason": format!("binary not found: {path}"),
            })
            .to_string();
        }

        let prefix = match wine_prefix() {
            Ok(p) => p,
            Err(e) => {
                return json!({ "started": false, "reason": e.to_string() }).to_string();
            }
        };

        // Serialise prefix bring-up. Once `system.reg` exists we can let
        // concurrent calls proceed in parallel; before that, wine can
        // step on itself if two `wineboot --init`s race.
        let initialised = prefix.join("system.reg").exists();
        if !initialised {
            let _gate = self.prefix_init.lock().await;
            // Double-check after acquiring — another waiter may have
            // finished while we were blocked.
            if !prefix.join("system.reg").exists() {
                tracing::info!(prefix = %prefix.display(), "Initialising Wine prefix");
                if let Err(e) = wineboot_init(&prefix).await {
                    return json!({
                        "started": false,
                        "reason": format!("prefix init failed: {e}"),
                    })
                    .to_string();
                }
            }
        }

        let mut cmd = tokio::process::Command::new(WINE_BINARY);
        cmd.env("WINEPREFIX", &prefix)
            .env("WINEARCH", WINE_ARCH)
            .arg(path)
            .args(&args);

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let reason = match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        "wine not installed (no `wine` in PATH)".to_string()
                    }
                    _ => format!("spawn wine: {e}"),
                };
                return json!({ "started": false, "reason": reason }).to_string();
            }
        };

        let pid = child.id().unwrap_or(0);
        tracing::info!(pid, %path, "Spawned wine process");

        // Watch the child in a background task so we can emit the
        // ProcessExited signal when it finishes. The DBus method
        // itself returns immediately.
        let ctx_owned = ctx.to_owned();
        tokio::spawn(async move {
            let mut child = child;
            let status = child.wait().await;
            let code = status.ok().and_then(|s| s.code()).unwrap_or(-1);
            tracing::info!(pid, exit = code, "Wine process exited");
            if let Err(e) = Self::process_exited(&ctx_owned, pid, code).await {
                tracing::warn!("ProcessExited emit failed: {e}");
            }
        });

        json!({ "started": true, "pid": pid }).to_string()
    }

    #[zbus(signal)]
    async fn process_exited(ctx: &SignalContext<'_>, pid: u32, status: i32) -> zbus::Result<()>;
}

fn wine_prefix() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no HOME directory"))?;
    Ok(home.join(".jarvis/wine/default"))
}

async fn wineboot_init(prefix: &std::path::Path) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(prefix).await?;
    let status = tokio::process::Command::new(WINE_BINARY)
        .env("WINEPREFIX", prefix)
        .env("WINEARCH", WINE_ARCH)
        .arg("wineboot")
        .arg("--init")
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("wineboot --init exited with {status}");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jarvis_compat=info".parse()?),
        )
        .init();

    tracing::info!("Starting Jarvis Compat");

    let service = CompatService {
        prefix_init: Arc::new(AsyncMutex::new(())),
    };

    let _conn = connection::Builder::session()?
        .name("com.jarvis.Compat")?
        .serve_at("/com/jarvis/Compat", service)?
        .build()
        .await?;

    tracing::info!("Compat ready on com.jarvis.Compat");

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wine_prefix_under_home() {
        // Best-effort — only meaningful when HOME is set. CI runners
        // typically have it; we degrade gracefully on the off chance
        // it isn't there.
        if let Ok(p) = wine_prefix() {
            let s = p.to_string_lossy();
            assert!(s.contains(".jarvis"), "{s}");
            assert!(s.ends_with("wine/default") || s.ends_with("wine\\default"));
        }
    }
}
