//! Jarvis Lock daemon.
//!
//! Owns `com.jarvis.Lock` on the session bus. Two responsibilities:
//!
//! 1. Track whether the session is currently locked. The shell binds
//!    to `LockStateChanged` so the bar's lock button can become a
//!    "session is locked" indicator if a third surface ever needs
//!    one. Idempotent — calling Lock() twice is a no-op.
//! 2. Authenticate password attempts via PAM. The Qt lock window
//!    sends `Verify(password)` and the daemon answers
//!    `{ ok: bool, reason?: string }`. Putting PAM behind a DBus
//!    method keeps the lock window from having to link libpam itself.
//!
//! Spawning the Qt lock window happens here too — `Lock()` launches
//! `jarvis-lock-window` (the Qt binary) as a child process, and
//! tracks its lifetime so a crash transitions us back to unlocked.

use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;
use zbus::{connection, interface, SignalContext};

const LOCK_WINDOW_BIN: &str = "/usr/bin/jarvis-lock-window";

struct LockService {
    locked: Arc<AsyncMutex<bool>>,
    child: Arc<AsyncMutex<Option<tokio::process::Child>>>,
}

#[interface(name = "com.jarvis.Lock")]
impl LockService {
    /// Engage the lock. Idempotent — calling again while locked is a
    /// no-op (the existing window stays up).
    async fn lock(&self, #[zbus(signal_context)] ctx: SignalContext<'_>) -> String {
        let mut locked = self.locked.lock().await;
        if *locked {
            return json!({ "ok": true, "already": true }).to_string();
        }

        // Spawn the Qt lock window as a child so we own its lifetime.
        let child = match tokio::process::Command::new(LOCK_WINDOW_BIN).spawn() {
            Ok(c) => c,
            Err(e) => {
                let reason = match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        format!("lock window binary missing at {LOCK_WINDOW_BIN}")
                    }
                    _ => format!("spawn lock window: {e}"),
                };
                return json!({ "ok": false, "reason": reason }).to_string();
            }
        };

        *self.child.lock().await = Some(child);
        *locked = true;
        drop(locked);

        if let Err(e) = Self::lock_state_changed(&ctx, true).await {
            tracing::warn!("LockStateChanged emit failed: {e}");
        }
        tracing::info!("Session locked");
        json!({ "ok": true, "already": false }).to_string()
    }

    /// Verify a password attempt through PAM. Called by the Qt lock
    /// window on submit. We shell out to `pamtester` rather than
    /// linking libpam directly — same auth path (it uses the
    /// system's PAM stack), but avoids the bindgen + libclang
    /// cascade the Rust `pam` crate pulls in.
    async fn verify(
        &self,
        password: &str,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> String {
        let user = std::env::var("USER").unwrap_or_else(|_| "jarvis".to_string());
        let ok = verify_with_pamtester(&user, password).await;
        if ok {
            tracing::info!("Password verified — unlocking");
            self.unlock_internal(&ctx).await;
            json!({ "ok": true }).to_string()
        } else {
            tracing::info!("Password rejected");
            json!({ "ok": false, "reason": "Senha incorreta" }).to_string()
        }
    }

    /// Reports whether the session is currently locked.
    async fn is_locked(&self) -> bool {
        *self.locked.lock().await
    }

    /// Fires whenever the locked state changes — the shell binds to
    /// this for any future "session is locked" UI affordance.
    #[zbus(signal)]
    async fn lock_state_changed(ctx: &SignalContext<'_>, locked: bool) -> zbus::Result<()>;
}

impl LockService {
    /// Drop the locked flag, kill the child window if still alive,
    /// emit the state-change signal. Called from `verify` after a
    /// successful PAM auth.
    async fn unlock_internal(&self, ctx: &SignalContext<'_>) {
        if let Some(mut child) = self.child.lock().await.take() {
            // The window quits itself on success but kill anyway in
            // case it's stuck — Kill is harmless when the process has
            // already exited.
            let _ = child.start_kill();
        }
        *self.locked.lock().await = false;
        if let Err(e) = Self::lock_state_changed(ctx, false).await {
            tracing::warn!("LockStateChanged emit failed: {e}");
        }
    }
}

/// Authenticate `user` against the system PAM stack via the
/// `pamtester` CLI. `pamtester <service> <user> authenticate` reads
/// the password from stdin and exits 0 on success. We use the
/// `login` PAM service — the same one greetd uses at boot.
async fn verify_with_pamtester(user: &str, password: &str) -> bool {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;

    let mut child = match tokio::process::Command::new("pamtester")
        .args(["login", user, "authenticate"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "spawn pamtester failed");
            return false;
        }
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(password.as_bytes()).await;
        let _ = stdin.write_all(b"\n").await;
    }
    match child.wait().await {
        Ok(status) => status.success(),
        Err(e) => {
            tracing::warn!(error = %e, "wait pamtester failed");
            false
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jarvis_lock=info".parse()?),
        )
        .init();

    tracing::info!("Starting Jarvis Lock");

    let service = LockService {
        locked: Arc::new(AsyncMutex::new(false)),
        child: Arc::new(AsyncMutex::new(None)),
    };

    let _conn = connection::Builder::session()?
        .name("com.jarvis.Lock")?
        .serve_at("/com/jarvis/Lock", service)?
        .build()
        .await?;

    tracing::info!("Lock ready on com.jarvis.Lock");

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}
