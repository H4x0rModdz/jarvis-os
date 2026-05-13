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
    /// window on submit. Uses the `jarvis-lock` PAM service which is
    /// password-only — no voice attempt, no added latency. Typed
    /// unlocks return as fast as the system's PAM stack allows.
    async fn verify(
        &self,
        password: &str,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> String {
        let user = std::env::var("USER").unwrap_or_else(|_| "jarvis".to_string());
        let ok = verify_with_pamtester("jarvis-lock", &user, Some(password)).await;
        if ok {
            tracing::info!("Password verified — unlocking");
            self.unlock_internal(&ctx).await;
            json!({ "ok": true }).to_string()
        } else {
            tracing::info!("Password rejected");
            json!({ "ok": false, "reason": "Senha incorreta" }).to_string()
        }
    }

    /// Verify a voice attempt through the dedicated voice PAM stack.
    /// Called by the Qt lock window's "Falar para desbloquear" pill.
    /// Goes through `jarvis-lock-voice` (pam_jarvis.so required) so
    /// the verdict is purely the voiceprint matcher's — the user
    /// already opted in by clicking, no password fallback on this
    /// path. Voice miss → returns ok:false; the lock window keeps
    /// the password field visible so the user can still type.
    async fn verify_voice(&self, #[zbus(signal_context)] ctx: SignalContext<'_>) -> String {
        let user = std::env::var("USER").unwrap_or_else(|_| "jarvis".to_string());
        let ok = verify_with_pamtester("jarvis-lock-voice", &user, None).await;
        if ok {
            tracing::info!("Voice verified — unlocking");
            self.unlock_internal(&ctx).await;
            json!({ "ok": true }).to_string()
        } else {
            tracing::info!("Voice rejected");
            json!({ "ok": false, "reason": "Voz não reconhecida" }).to_string()
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

/// Authenticate `user` against the given PAM `service` via the
/// `pamtester` CLI. When `password` is `Some`, it's piped through
/// stdin so the password-path service can read it via pam_unix.
/// When `None`, no stdin is fed — the voice service doesn't expect
/// any. Returns whether `pamtester` exited 0.
///
/// Two services in play:
///   `jarvis-lock`       — password-only, fast path (no voice attempt)
///   `jarvis-lock-voice` — voiceprint required, no password fallback
///
/// See ADR 0020 (amended for Phase 8) for the split rationale.
async fn verify_with_pamtester(service: &str, user: &str, password: Option<&str>) -> bool {
    use std::process::Stdio;
    use tokio::io::AsyncWriteExt;

    let stdin_kind = if password.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };

    let mut child = match tokio::process::Command::new("pamtester")
        .args([service, user, "authenticate"])
        .stdin(stdin_kind)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, service, "spawn pamtester failed");
            return false;
        }
    };
    if let (Some(pw), Some(mut stdin)) = (password, child.stdin.take()) {
        let _ = stdin.write_all(pw.as_bytes()).await;
        let _ = stdin.write_all(b"\n").await;
    }
    match child.wait().await {
        Ok(status) => status.success(),
        Err(e) => {
            tracing::warn!(error = %e, service, "wait pamtester failed");
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

    let conn = connection::Builder::session()?
        .name("com.jarvis.Lock")?
        .serve_at("/com/jarvis/Lock", service)?
        .build()
        .await?;

    tracing::info!("Lock ready on com.jarvis.Lock");

    // Auto-lock supervisor. The old labwc autostart used a hardcoded
    // swayidle invocation; now the lock daemon owns it and respawns
    // swayidle whenever `lock.idle_timeout_seconds` changes in
    // com.jarvis.Settings. Off when timeout == 0.
    let conn_for_supervisor = conn.clone();
    tokio::spawn(async move {
        if let Err(e) = idle_lock_supervisor(conn_for_supervisor).await {
            tracing::warn!(error = %e, "idle-lock supervisor exited");
        }
    });

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}

/// Reads `lock.idle_timeout_seconds` from com.jarvis.Settings, spawns
/// swayidle with that timeout, and respawns when the setting changes.
/// Treats Settings being unreachable as "use the default" rather than
/// crashing — boot races between lock and settings are normal.
async fn idle_lock_supervisor(conn: zbus::Connection) -> anyhow::Result<()> {
    use futures_util::stream::StreamExt;

    const DEFAULT_TIMEOUT: u64 = 300;
    const SETTINGS_KEY: &str = "lock.idle_timeout_seconds";

    // Wait briefly for Settings to come up — typical race at session
    // start, both daemons are Wants= on jarvis-session.target.
    let proxy = loop {
        match zbus::Proxy::new(
            &conn,
            "com.jarvis.Settings",
            "/com/jarvis/Settings",
            "com.jarvis.Settings",
        )
        .await
        {
            Ok(p) => match p.call::<_, _, String>("Get", &("__ping__",)).await {
                Ok(_) => break p,
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(500)).await,
            },
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(500)).await,
        }
    };

    let mut current: Option<tokio::process::Child> = None;
    let mut active_timeout: u64 = 0;

    let apply = |seconds: u64, current: &mut Option<tokio::process::Child>, active: &mut u64| {
        if *active == seconds {
            return;
        }
        if let Some(mut prev) = current.take() {
            let _ = prev.start_kill();
        }
        *active = seconds;
        if seconds == 0 {
            tracing::info!("Auto-lock disabled (timeout=0)");
            return;
        }
        tracing::info!(seconds, "Spawning swayidle");
        match tokio::process::Command::new("swayidle")
            .args([
                "-w",
                "timeout",
                &seconds.to_string(),
                "jarvis-lock-ctl lock",
            ])
            .spawn()
        {
            Ok(c) => *current = Some(c),
            Err(e) => tracing::warn!(error = %e, "swayidle spawn failed"),
        }
    };

    // Initial read.
    let initial = read_timeout_seconds(&proxy, SETTINGS_KEY)
        .await
        .unwrap_or(DEFAULT_TIMEOUT);
    apply(initial, &mut current, &mut active_timeout);

    // Subscribe to Changed signals. zbus 4 returns a signal stream;
    // filter to our key only.
    let mut stream = proxy.receive_signal("Changed").await?;
    while let Some(msg) = stream.next().await {
        let Ok((key, _value_json)) = msg.body().deserialize::<(String, String)>() else {
            continue;
        };
        if key != SETTINGS_KEY {
            continue;
        }
        let seconds = read_timeout_seconds(&proxy, SETTINGS_KEY)
            .await
            .unwrap_or(DEFAULT_TIMEOUT);
        apply(seconds, &mut current, &mut active_timeout);
    }
    Ok(())
}

/// Read a number from Settings. The store holds JSON strings, so we
/// parse and coerce. None means key missing / malformed / wrong type
/// — caller falls back to its own default.
async fn read_timeout_seconds(proxy: &zbus::Proxy<'_>, key: &str) -> Option<u64> {
    let resp: String = proxy.call("Get", &(key,)).await.ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&resp).ok()?;
    if !parsed.get("found")?.as_bool()? {
        return None;
    }
    parsed.get("value")?.as_u64()
}
