use crate::action_bus::WindowCommand;
use calloop::{channel::Sender, LoopSignal};
use smithay::{
    desktop::{Space, Window},
    input::{pointer::CursorImageStatus, Seat, SeatState},
    reexports::wayland_server::{
        backend::{ClientData, ClientId, DisconnectReason},
        DisplayHandle,
    },
    utils::{Clock, Logical, Monotonic, Point},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        output::OutputManagerState,
        shell::{
            wlr_layer::WlrLayerShellState,
            xdg::XdgShellState,
        },
        shm::ShmState,
    },
};
use std::collections::HashMap;

/// Per-window metadata tracked by the compositor.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub id: u32,
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub minimized: bool,
}

/// The central state of the Jarvis compositor.
///
/// Owns all Smithay protocol state and Jarvis-specific state.
/// Passed through the calloop event loop and mutated by event handlers.
pub struct JarvisCompositor {
    // ── Core ─────────────────────────────────────────────────────────────
    pub display_handle: DisplayHandle,
    pub loop_signal: LoopSignal,
    pub clock: Clock<Monotonic>,

    // ── Wayland protocol state ────────────────────────────────────────────
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub layer_shell_state: WlrLayerShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,

    // ── Input ─────────────────────────────────────────────────────────────
    pub seat_state: SeatState<Self>,
    pub seat: Seat<Self>,
    pub pointer_location: Point<f64, Logical>,
    pub cursor_status: CursorImageStatus,

    // ── Window management ─────────────────────────────────────────────────
    /// The Smithay space — tracks window positions and z-order.
    pub space: Space<Window>,
    /// Maps our u32 window IDs to Smithay window objects.
    pub windows: HashMap<u32, Window>,
    /// Maps Smithay window objects back to our IDs (by pointer identity).
    pub window_ids: HashMap<usize, u32>,
    pub next_window_id: u32,

    // ── Action Bus bridge ─────────────────────────────────────────────────
    /// Receives WindowCommands from the Action Bus listener thread.
    pub cmd_tx: Option<Sender<WindowCommand>>,
}

impl JarvisCompositor {
    pub fn allocate_window_id(&mut self) -> u32 {
        let id = self.next_window_id;
        self.next_window_id += 1;
        id
    }

    pub fn window_by_id(&self, id: u32) -> Option<&Window> {
        self.windows.get(&id)
    }

    pub fn focus_window(&mut self, id: u32) {
        if let Some(window) = self.windows.get(&id) {
            let window = window.clone();
            self.space.raise_element(&window, true);
            // Keyboard focus is set in the seat handler
        } else {
            tracing::warn!(window_id = id, "Focus requested for unknown window");
        }
    }

    pub fn minimize_window(&mut self, id: u32) {
        // Minimized windows are unmapped from the space but kept in self.windows
        if let Some(window) = self.windows.get(&id) {
            self.space.unmap_elem(window);
            tracing::info!(window_id = id, "Window minimized");
        }
    }

    pub fn close_window(&mut self, id: u32, force: bool) {
        if let Some(window) = self.windows.get(&id) {
            if force {
                // TODO: find PID and send SIGKILL via Action Bus
                tracing::warn!(window_id = id, "Force close not yet implemented");
            } else {
                window.toplevel().map(|t| t.send_close());
            }
        }
    }
}

/// Per-client data stored in the Wayland client object.
#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
