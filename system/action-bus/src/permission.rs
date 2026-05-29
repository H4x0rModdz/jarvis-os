use crate::action::ActionRequest;
use crate::error::BusError;
use serde::Deserialize;
use zbus::{proxy, Connection};

/// Enforces permission scopes for every dispatched action.
///
/// Calls `com.jarvis.PermissionSystem.Check` over the session bus. If the
/// daemon is unreachable, falls back to a local safe-vs-dangerous policy
/// — safe scopes stay allowed so the system isn't bricked when the
/// permission daemon is restarting; dangerous scopes are denied.
pub struct PermissionChecker {
    proxy: Option<PermissionSystemProxy<'static>>,
    /// Test-only bypass. Always false in production; tests use [`allow_all`]
    /// to skip the policy lookup so they can use synthetic action names.
    bypass: bool,
}

#[proxy(
    interface = "com.jarvis.PermissionSystem",
    default_service = "com.jarvis.PermissionSystem",
    default_path = "/com/jarvis/PermissionSystem"
)]
trait PermissionSystem {
    fn check(&self, caller: &str, scope: &str, action: &str) -> zbus::Result<String>;
}

#[derive(Debug, Deserialize)]
struct CheckResponse {
    outcome: String,
    approved_by: String,
}

impl PermissionChecker {
    /// Try to wire up the DBus proxy. Returns a checker either way — if the
    /// permission daemon isn't on the bus yet, `check()` will use the local
    /// fallback policy.
    pub async fn new() -> Self {
        let proxy = match Connection::session().await {
            Ok(conn) => match PermissionSystemProxy::new(&conn).await {
                Ok(p) => Some(p),
                Err(e) => {
                    tracing::warn!("PermissionSystem proxy unavailable: {e}");
                    None
                }
            },
            Err(e) => {
                tracing::warn!("Session bus unavailable: {e}");
                None
            }
        };
        Self {
            proxy,
            bypass: false,
        }
    }

    #[cfg(test)]
    pub fn allow_all() -> Self {
        Self {
            proxy: None,
            bypass: true,
        }
    }

    pub async fn check(&self, request: &ActionRequest) -> Result<(), BusError> {
        if self.bypass {
            return Ok(());
        }
        let scope = Self::required_scope(&request.action);
        let caller = request.caller.to_string();

        if let Some(proxy) = &self.proxy {
            match proxy.check(&caller, scope, &request.action).await {
                Ok(json) => match serde_json::from_str::<CheckResponse>(&json) {
                    Ok(resp) => {
                        tracing::info!(
                            action = %request.action,
                            caller = %caller,
                            scope,
                            outcome = %resp.outcome,
                            approved_by = %resp.approved_by,
                            "Permission check (daemon)"
                        );
                        if resp.outcome == "approved" {
                            return Ok(());
                        }
                        return Err(BusError::PermissionDenied {
                            scope: format!("{scope} ({})", resp.approved_by),
                        });
                    }
                    Err(e) => {
                        tracing::warn!("PermissionSystem returned non-JSON: {e} — falling back");
                    }
                },
                Err(e) => {
                    tracing::warn!("PermissionSystem call failed: {e} — falling back");
                }
            }
        }

        // Fallback policy: safe scopes are allowed, dangerous ones denied.
        if Self::is_safe_scope(scope) {
            tracing::debug!(
                action = %request.action,
                scope,
                "Permission check (fallback: safe scope auto-allowed)"
            );
            Ok(())
        } else {
            tracing::warn!(
                action = %request.action,
                scope,
                "Permission check (fallback: dangerous scope denied — daemon offline)"
            );
            Err(BusError::PermissionDenied {
                scope: format!("{scope} (daemon offline)"),
            })
        }
    }

    fn required_scope(action: &str) -> &'static str {
        match action {
            "app.open" => "app.launch",
            "app.close" => "app.launch",
            "app.install" => "app.install",
            "app.uninstall" => "app.uninstall",
            "file.move" => "filesystem.write",
            "file.copy" => "filesystem.write",
            "file.delete" => "filesystem.delete",
            "window.focus" | "window.minimize" | "window.maximize" | "window.close"
            | "window.move" | "window.resize" | "window.snap_left" | "window.snap_right" => {
                "window.control"
            }
            "workspace.switch" | "workspace.move_window" | "workspace.create" => "window.control",
            "system.notify" => "system.notify",
            "system.set_setting" => "settings.modify",
            "system.get_setting" => "settings.read",
            // Power ops are sensitive: NOT a safe scope, so a non-menu
            // (e.g. Lilith-initiated) call prompts for confirmation.
            "system.power" => "system.power",
            _ => "unknown",
        }
    }

    /// Mirrors the safe-scope list in `system/permission/src/policy.rs`. Kept
    /// duplicated rather than shared to avoid a workspace-internal crate for
    /// what is currently five strings; revisit if the list grows.
    fn is_safe_scope(scope: &str) -> bool {
        const SAFE: &[&str] = &[
            "app.launch",
            "window.control",
            "system.notify",
            "settings.read",
            "filesystem.read",
        ];
        SAFE.iter()
            .any(|prefix| scope == *prefix || scope.starts_with(&format!("{prefix}.")))
    }
}
