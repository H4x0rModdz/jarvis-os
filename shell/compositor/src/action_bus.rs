#![allow(dead_code)]

use calloop::channel::Sender;
use serde::Serialize;
use zbus::Connection;

/// Commands the Action Bus sends to the compositor.
#[derive(Debug, Clone)]
pub enum WindowCommand {
    Focus {
        window_id: u32,
    },
    Minimize {
        window_id: u32,
    },
    Maximize {
        window_id: u32,
    },
    Close {
        window_id: u32,
        force: bool,
    },
    Move {
        window_id: u32,
        x: i32,
        y: i32,
    },
    Resize {
        window_id: u32,
        width: u32,
        height: u32,
    },
    SnapLeft {
        window_id: u32,
    },
    SnapRight {
        window_id: u32,
    },
}

/// Events the compositor emits to notify the rest of the system.
#[derive(Debug, Clone, Serialize)]
pub struct WindowEvent {
    pub event: &'static str,
    pub window_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Spawns a tokio thread that:
/// 1. Connects to the Action Bus via DBus
/// 2. Listens for window.* action requests
/// 3. Forwards them to the compositor event loop via a calloop channel
pub fn spawn_action_bus_listener(cmd_tx: Sender<WindowCommand>) {
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(listen_loop(cmd_tx));
    });
}

async fn listen_loop(_cmd_tx: Sender<WindowCommand>) {
    let _conn = match Connection::session().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Action Bus DBus connection failed: {e} — window actions disabled");
            return;
        }
    };

    tracing::info!("Compositor connected to DBus session bus");

    // Subscribe to the WindowActionRequested signal from the Action Bus.
    // The Action Bus emits this signal when it receives a window.* action
    // that needs to be handled by the compositor.
    //
    // TODO: implement the signal subscription once the Action Bus
    // emits WindowActionRequested signals. For now, the channel is
    // open and ready to receive commands from any future source.

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

/// Parse an action.* JSON payload into a WindowCommand.
/// Called by the DBus signal handler when a window action arrives.
pub fn parse_window_command(action: &str, params: &serde_json::Value) -> Option<WindowCommand> {
    let window_id = params["window_id"].as_u64()? as u32;

    match action {
        "window.focus" => Some(WindowCommand::Focus { window_id }),
        "window.minimize" => Some(WindowCommand::Minimize { window_id }),
        "window.maximize" => Some(WindowCommand::Maximize { window_id }),
        "window.close" => Some(WindowCommand::Close {
            window_id,
            force: params["force"].as_bool().unwrap_or(false),
        }),
        "window.move" => Some(WindowCommand::Move {
            window_id,
            x: params["x"].as_i64()? as i32,
            y: params["y"].as_i64()? as i32,
        }),
        "window.resize" => Some(WindowCommand::Resize {
            window_id,
            width: params["width"].as_u64()? as u32,
            height: params["height"].as_u64()? as u32,
        }),
        "window.snap_left" => Some(WindowCommand::SnapLeft { window_id }),
        "window.snap_right" => Some(WindowCommand::SnapRight { window_id }),
        _ => None,
    }
}
