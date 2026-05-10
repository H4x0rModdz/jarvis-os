mod policy;
mod store;

use policy::{classify, PolicyVerdict};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use store::GrantStore;
use tokio::sync::{oneshot, Mutex as AsyncMutex};
use uuid::Uuid;
use zbus::{connection, interface, SignalContext};

/// How long `Check()` blocks waiting for the user to resolve an approval
/// before auto-denying. Long enough that the user has time to read the
/// dialog, short enough that a forgotten / dismissed prompt doesn't pin a
/// caller's request forever.
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(30);

struct PermissionService {
    store: Arc<GrantStore>,
    /// Outstanding approval requests waiting for the UI's decision.
    pending: Arc<AsyncMutex<HashMap<String, oneshot::Sender<Decision>>>>,
}

#[derive(Debug, Clone, Copy)]
enum Decision {
    /// Approve this single Check() call. No grant stored.
    Approve,
    /// Approve and also persist a grant so future calls auto-allow.
    ApprovePersistent,
    /// Deny this Check() call.
    Deny,
}

#[derive(Debug, Serialize)]
struct CheckResult {
    outcome: String,
    approved_by: String,
}

#[interface(name = "com.jarvis.PermissionSystem")]
impl PermissionService {
    /// Decide whether `caller` may perform `action` under `scope`.
    /// Returns a JSON `{ outcome, approved_by }`.
    ///
    /// For dangerous scopes without an existing grant this emits the
    /// `ApprovalRequested` signal and blocks (up to `APPROVAL_TIMEOUT`)
    /// waiting for a `ResolveApproval` call. If nobody resolves, the
    /// outcome is `denied` with `approved_by = "timeout"`.
    async fn check(
        &self,
        caller: &str,
        scope: &str,
        action: &str,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
    ) -> String {
        let immediate = self.evaluate(caller, scope, action);

        let result = if immediate.approved_by == "policy:requires_grant" {
            self.request_approval(caller, scope, action, &ctx).await
        } else {
            immediate
        };

        tracing::info!(
            caller,
            scope,
            action,
            outcome = %result.outcome,
            approved_by = %result.approved_by,
            "Permission check"
        );
        serde_json::to_string(&result).unwrap_or_else(|_| "{}".into())
    }

    /// Resolve a pending approval request. Called by the UI when the user
    /// clicks Allow / Allow Always / Deny on an approval dialog.
    ///
    /// `decision` is one of: "approve", "approve_persistent", "deny".
    /// Returns `{ "ok": bool, "error": string? }`.
    async fn resolve_approval(&self, request_id: &str, decision: &str) -> String {
        let parsed = match decision {
            "approve" => Decision::Approve,
            "approve_persistent" => Decision::ApprovePersistent,
            "deny" => Decision::Deny,
            _ => {
                return json!({ "ok": false, "error": "unknown decision" }).to_string();
            }
        };

        let sender = self.pending.lock().await.remove(request_id);
        match sender {
            Some(tx) => {
                // If the waiter already gave up (timeout), the send fails.
                // That's fine — we just drop the decision.
                let _ = tx.send(parsed);
                tracing::info!(request_id, decision, "Approval resolved");
                json!({ "ok": true }).to_string()
            }
            None => {
                tracing::warn!(
                    request_id,
                    "Unknown approval id (already resolved or timed out)"
                );
                json!({ "ok": false, "error": "unknown request_id" }).to_string()
            }
        }
    }

    /// Pre-authorize a (caller, scope) pair. Useful for tests and headless
    /// setups without an approval UI.
    async fn grant(&self, caller: &str, scope: &str, persistent: bool) -> String {
        let grant = self.store.grant(caller, scope, persistent);
        tracing::info!(caller, scope, persistent, "Grant added");
        serde_json::to_string(&grant).unwrap_or_else(|_| "{}".into())
    }

    /// Remove an existing grant. Returns `{ revoked: true|false }`.
    async fn revoke(&self, caller: &str, scope: &str) -> String {
        let removed = self.store.revoke(caller, scope);
        tracing::info!(caller, scope, removed, "Grant revoked");
        json!({ "revoked": removed }).to_string()
    }

    /// Return every active grant as a JSON array.
    async fn list_grants(&self) -> String {
        let grants = self.store.list();
        serde_json::to_string(&grants).unwrap_or_else(|_| "[]".into())
    }

    /// Emitted when a dangerous scope is requested without an existing
    /// grant. The UI should display a prompt and respond by calling
    /// `ResolveApproval(request_id, decision)`.
    #[zbus(signal)]
    async fn approval_requested(
        ctx: &SignalContext<'_>,
        request_id: &str,
        caller: &str,
        scope: &str,
        action: &str,
    ) -> zbus::Result<()>;
}

impl PermissionService {
    fn evaluate(&self, caller: &str, scope: &str, _action: &str) -> CheckResult {
        if caller.is_empty() || scope.is_empty() {
            return CheckResult {
                outcome: "denied".into(),
                approved_by: "policy:malformed".into(),
            };
        }

        match classify(scope) {
            PolicyVerdict::AutoAllow => CheckResult {
                outcome: "approved".into(),
                approved_by: "policy:safe_scope".into(),
            },
            PolicyVerdict::RequireGrant => {
                if self.store.has(caller, scope) {
                    CheckResult {
                        outcome: "approved".into(),
                        approved_by: "grant".into(),
                    }
                } else {
                    CheckResult {
                        outcome: "denied".into(),
                        approved_by: "policy:requires_grant".into(),
                    }
                }
            }
            PolicyVerdict::Unknown => CheckResult {
                outcome: "denied".into(),
                approved_by: "policy:unknown_scope".into(),
            },
        }
    }

    async fn request_approval(
        &self,
        caller: &str,
        scope: &str,
        action: &str,
        ctx: &SignalContext<'_>,
    ) -> CheckResult {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();

        // Register pending BEFORE emitting the signal so a fast UI can't
        // race us to ResolveApproval.
        self.pending.lock().await.insert(id.clone(), tx);

        if let Err(e) = Self::approval_requested(ctx, &id, caller, scope, action).await {
            tracing::warn!("Failed to emit ApprovalRequested: {e}");
            self.pending.lock().await.remove(&id);
            return CheckResult {
                outcome: "denied".into(),
                approved_by: "signal_failed".into(),
            };
        }

        tracing::info!(request_id = %id, caller, scope, action, "Approval requested");

        match tokio::time::timeout(APPROVAL_TIMEOUT, rx).await {
            Ok(Ok(Decision::Approve)) => CheckResult {
                outcome: "approved".into(),
                approved_by: "approval:once".into(),
            },
            Ok(Ok(Decision::ApprovePersistent)) => {
                self.store.grant(caller, scope, true);
                CheckResult {
                    outcome: "approved".into(),
                    approved_by: "approval:persistent".into(),
                }
            }
            Ok(Ok(Decision::Deny)) => CheckResult {
                outcome: "denied".into(),
                approved_by: "approval:user".into(),
            },
            Ok(Err(_)) => {
                // Sender dropped without sending — shouldn't happen in
                // normal flow but treat as a deny.
                CheckResult {
                    outcome: "denied".into(),
                    approved_by: "approval:cancelled".into(),
                }
            }
            Err(_) => {
                // Timeout — purge pending entry so resolve_approval gets a
                // clean "unknown id" if it eventually arrives.
                self.pending.lock().await.remove(&id);
                CheckResult {
                    outcome: "denied".into(),
                    approved_by: "approval:timeout".into(),
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jarvis_permission=info".parse()?),
        )
        .init();

    tracing::info!("Starting Jarvis Permission System");

    let service = PermissionService {
        store: Arc::new(GrantStore::new()),
        pending: Arc::new(AsyncMutex::new(HashMap::new())),
    };

    let _conn = connection::Builder::session()?
        .name("com.jarvis.PermissionSystem")?
        .serve_at("/com/jarvis/PermissionSystem", service)?
        .build()
        .await?;

    tracing::info!("Permission System ready on com.jarvis.PermissionSystem");

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svc() -> PermissionService {
        PermissionService {
            store: Arc::new(GrantStore::new()),
            pending: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }

    #[test]
    fn safe_scope_auto_allowed() {
        let s = svc();
        let r = s.evaluate("lilith", "app.launch", "app.open");
        assert_eq!(r.outcome, "approved");
        assert_eq!(r.approved_by, "policy:safe_scope");
    }

    #[test]
    fn dangerous_scope_denied_without_grant() {
        let s = svc();
        let r = s.evaluate("lilith", "filesystem.delete", "file.delete");
        assert_eq!(r.outcome, "denied");
        assert_eq!(r.approved_by, "policy:requires_grant");
    }

    #[test]
    fn dangerous_scope_allowed_after_grant() {
        let s = svc();
        s.store.grant("lilith", "filesystem.delete", false);
        let r = s.evaluate("lilith", "filesystem.delete", "file.delete");
        assert_eq!(r.outcome, "approved");
        assert_eq!(r.approved_by, "grant");
    }

    #[test]
    fn grant_is_caller_scoped() {
        let s = svc();
        s.store.grant("lilith", "app.install", false);
        let r = s.evaluate("app:firefox", "app.install", "app.install");
        assert_eq!(r.outcome, "denied");
    }

    #[test]
    fn unknown_scope_denied() {
        let s = svc();
        let r = s.evaluate("lilith", "totally.fake.scope", "action.x");
        assert_eq!(r.outcome, "denied");
        assert_eq!(r.approved_by, "policy:unknown_scope");
    }

    #[test]
    fn empty_caller_or_scope_denied() {
        let s = svc();
        assert_eq!(s.evaluate("", "app.launch", "app.open").outcome, "denied");
        assert_eq!(s.evaluate("lilith", "", "app.open").outcome, "denied");
    }

    /// Simulates the approval flow without DBus: bypasses signal emission
    /// by directly inserting a pending entry, resolving it, and asserting
    /// the right outcome. The signal-context-requiring `request_approval`
    /// is exercised via the dedicated e2e test script.
    #[tokio::test]
    async fn pending_resolve_approve_once() {
        let s = svc();
        let id = "test-id-1".to_string();
        let (tx, rx) = oneshot::channel::<Decision>();
        s.pending.lock().await.insert(id.clone(), tx);

        let resolved = s.resolve_approval(&id, "approve").await;
        assert!(resolved.contains("\"ok\":true"));

        let decision = tokio::time::timeout(Duration::from_millis(100), rx)
            .await
            .expect("decision received")
            .expect("channel ok");
        assert!(matches!(decision, Decision::Approve));
        assert!(s.pending.lock().await.is_empty());
    }

    #[tokio::test]
    async fn pending_resolve_persistent_stores_grant() {
        let s = svc();
        let id = "test-id-2".to_string();
        let (tx, rx) = oneshot::channel::<Decision>();
        s.pending.lock().await.insert(id.clone(), tx);

        // Simulate the upstream pattern: caller waits on rx and, on receiving
        // ApprovePersistent, writes a grant. We replicate that here.
        s.resolve_approval(&id, "approve_persistent").await;
        let decision = rx.await.unwrap();
        if matches!(decision, Decision::ApprovePersistent) {
            s.store.grant("lilith", "filesystem.delete", true);
        }
        assert!(s.store.has("lilith", "filesystem.delete"));
    }

    #[tokio::test]
    async fn resolve_unknown_id_returns_error() {
        let s = svc();
        let r = s.resolve_approval("does-not-exist", "approve").await;
        assert!(r.contains("\"ok\":false"));
        assert!(r.contains("unknown request_id"));
    }

    #[tokio::test]
    async fn resolve_invalid_decision_returns_error() {
        let s = svc();
        let id = "test-id-3".to_string();
        let (tx, _rx) = oneshot::channel::<Decision>();
        s.pending.lock().await.insert(id.clone(), tx);

        let r = s.resolve_approval(&id, "garbage").await;
        assert!(r.contains("unknown decision"));
        // Pending entry should still exist — invalid decision doesn't consume it.
        assert!(s.pending.lock().await.contains_key(&id));
    }
}
