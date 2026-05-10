use crate::action::ActionRequest;
use crate::error::BusError;

/// Enforces permission scopes for every dispatched action.
///
/// Currently a stub — always allows. Will call the Permission System daemon
/// (com.jarvis.PermissionSystem) once that module is built.
pub struct PermissionChecker;

impl PermissionChecker {
    pub fn new() -> Self {
        Self
    }

    pub async fn check(&self, request: &ActionRequest) -> Result<(), BusError> {
        let scope = Self::required_scope(&request.action);
        tracing::debug!(
            action = %request.action,
            caller = %request.caller,
            scope,
            "Permission check (stub — always allowed)"
        );
        // TODO: call com.jarvis.PermissionSystem.Check(caller, scope)
        Ok(())
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
            _ => "unknown",
        }
    }
}
