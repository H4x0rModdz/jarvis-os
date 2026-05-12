mod action;
mod audit;
mod bus;
mod error;
mod handlers;
mod permission;
mod registry;

use action::ActionRequest;
use audit::AuditLog;
use bus::ActionBus;
use registry::{HandlerFuture, Registry};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use zbus::{connection, interface};

struct ActionBusService {
    bus: Arc<ActionBus>,
}

#[interface(name = "com.jarvis.ActionBus")]
impl ActionBusService {
    /// Dispatch an action. Always returns a JSON response — never fails at DBus level.
    async fn dispatch(&self, action_json: &str) -> String {
        let request: ActionRequest = match serde_json::from_str(action_json) {
            Ok(r) => r,
            Err(e) => {
                return serde_json::json!({
                    "action": "unknown",
                    "status": "error",
                    "error": { "code": "INVALID_PARAMS", "message": e.to_string() },
                    "duration_ms": 0
                })
                .to_string();
            }
        };

        let response = self.bus.dispatch(request).await;
        serde_json::to_string(&response).unwrap_or_else(|_| "{}".into())
    }

    /// List all registered action names. Used by Lilith for capability discovery.
    async fn list_actions(&self) -> Vec<String> {
        self.bus.list_actions().await
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jarvis_action_bus=info".parse()?),
        )
        .init();

    let audit_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".jarvis/logs/action-bus.log");

    tracing::info!("Starting Jarvis Action Bus");
    tracing::info!("Audit log: {}", audit_path.display());

    let audit = AuditLog::new(audit_path);
    let registry = build_registry();
    let action_count = registry.action_names().len();
    let bus = Arc::new(ActionBus::new(registry, audit).await);

    let service = ActionBusService { bus };

    let _conn = connection::Builder::session()?
        .name("com.jarvis.ActionBus")?
        .serve_at("/com/jarvis/ActionBus", service)?
        .build()
        .await?;

    tracing::info!(
        actions = action_count,
        "Action Bus ready on com.jarvis.ActionBus"
    );

    // Keep the daemon alive
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}

fn build_registry() -> Registry {
    let mut r = Registry::new();

    macro_rules! reg {
        ($action:expr, $handler:expr) => {
            r.register(
                $action,
                Arc::new(|params: Value| Box::pin($handler(params)) as HandlerFuture),
            )
        };
    }

    reg!("app.open", handlers::app::open);
    reg!("app.close", handlers::app::close);
    reg!("app.install", handlers::app::install);
    reg!("app.uninstall", handlers::app::uninstall);

    reg!("file.move", handlers::file::move_file);
    reg!("file.copy", handlers::file::copy_file);
    reg!("file.delete", handlers::file::delete);

    reg!("window.focus", handlers::window::focus);
    reg!("window.minimize", handlers::window::minimize);
    reg!("window.maximize", handlers::window::maximize);
    reg!("window.close", handlers::window::close);
    reg!("window.move", handlers::window::move_window);
    reg!("window.resize", handlers::window::resize);
    reg!("window.snap_left", handlers::window::snap_left);
    reg!("window.snap_right", handlers::window::snap_right);

    reg!("workspace.switch", handlers::workspace::switch);
    reg!("workspace.move_window", handlers::workspace::move_window);
    reg!("workspace.create", handlers::workspace::create);

    reg!("system.notify", handlers::system::notify);
    reg!("system.set_setting", handlers::system::set_setting);
    reg!("system.get_setting", handlers::system::get_setting);

    reg!("browser.open", handlers::browser::open);

    reg!("clipboard.set", handlers::clipboard::set);
    reg!("clipboard.get", handlers::clipboard::get);

    reg!("screenshot.capture", handlers::screenshot::capture);

    reg!("audio.set_volume", handlers::audio::set_volume);
    reg!("audio.adjust_volume", handlers::audio::adjust_volume);
    reg!("audio.toggle_mute", handlers::audio::toggle_mute);

    reg!("updater.check", handlers::updater::check);
    reg!("updater.apply_os", handlers::updater::apply_os);

    reg!("compat.run_exe", handlers::compat::run_exe);
    reg!("compat.run_exe_in", handlers::compat::run_exe_in);
    reg!("compat.create_prefix", handlers::compat::create_prefix);
    reg!("compat.list_prefixes", handlers::compat::list_prefixes);

    // SDK apps: pick up every manifest under /usr/share/jarvis/apps/
    // and ~/.local/share/jarvis/apps/ and register their declared
    // actions as proxy handlers. Built-in handlers always win on
    // conflict because they're registered first.
    let scan_paths = jarvis_sdk_types::default_scan_paths();
    let added = handlers::sdk::register_all(&mut r, scan_paths);
    if added > 0 {
        tracing::info!(actions = added, "Registered SDK app actions");
    }

    r
}
