mod policy;
mod store;

use policy::{classify, PolicyVerdict};
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use store::GrantStore;
use zbus::{connection, interface};

struct PermissionService {
    store: Arc<GrantStore>,
}

#[derive(Debug, Serialize)]
struct CheckResult<'a> {
    outcome: &'a str,
    approved_by: &'a str,
}

#[interface(name = "com.jarvis.PermissionSystem")]
impl PermissionService {
    /// Decide whether `caller` may perform `action` under `scope`.
    /// Returns a JSON `{ outcome, approved_by }`.
    async fn check(&self, caller: &str, scope: &str, action: &str) -> String {
        let result = self.evaluate(caller, scope, action);
        tracing::info!(
            caller,
            scope,
            action,
            outcome = result.outcome,
            approved_by = result.approved_by,
            "Permission check"
        );
        serde_json::to_string(&result).unwrap_or_else(|_| "{}".into())
    }

    /// Pre-authorize a (caller, scope) pair. Phase 1 has no UI prompt — this
    /// is the only way to authorize a dangerous scope.
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
}

impl PermissionService {
    fn evaluate<'a>(&self, caller: &str, scope: &str, _action: &str) -> CheckResult<'a> {
        if caller.is_empty() || scope.is_empty() {
            return CheckResult {
                outcome: "denied",
                approved_by: "policy:malformed",
            };
        }

        match classify(scope) {
            PolicyVerdict::AutoAllow => CheckResult {
                outcome: "approved",
                approved_by: "policy:safe_scope",
            },
            PolicyVerdict::RequireGrant => {
                if self.store.has(caller, scope) {
                    CheckResult {
                        outcome: "approved",
                        approved_by: "grant",
                    }
                } else {
                    CheckResult {
                        outcome: "denied",
                        approved_by: "policy:requires_grant",
                    }
                }
            }
            PolicyVerdict::Unknown => CheckResult {
                outcome: "denied",
                approved_by: "policy:unknown_scope",
            },
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
        // a different caller should NOT be auto-allowed
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
}
