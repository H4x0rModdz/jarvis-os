use crate::state::JarvisCompositor;
use smithay::{
    delegate_xdg_shell,
    desktop::Window,
    reexports::wayland_server::protocol::wl_seat::WlSeat,
    utils::{Serial, SERIAL_COUNTER},
    wayland::shell::xdg::{
        PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
        XdgToplevelSurfaceData,
    },
};

impl XdgShellHandler for JarvisCompositor {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);
        let id = self.allocate_window_id();

        // Place window at a sensible default position
        let position = self.next_window_position();
        self.space.map_element(window.clone(), position, true);

        // Track it
        let ptr_id = window_ptr_id(&window);
        self.windows.insert(id, window.clone());
        self.window_ids.insert(ptr_id, id);

        let app_id = window
            .toplevel()
            .and_then(|t| t.with_pending_state(|s| s.app_id.clone()));
        let title = window
            .toplevel()
            .and_then(|t| t.with_pending_state(|s| s.title.clone()));

        tracing::info!(window_id = id, ?app_id, ?title, "New toplevel window");
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        // Popups are positioned by the client — we just track them
        self.unconstrain_popup(&surface);
    }

    fn move_request(&mut self, surface: ToplevelSurface, _seat: WlSeat, _serial: Serial) {
        let window = self.window_for_surface(surface.wl_surface());
        if let Some(_window) = window {
            // TODO: begin interactive move — track pointer delta and reposition
            tracing::debug!("Interactive move requested");
        }
    }

    fn resize_request(
        &mut self,
        _surface: ToplevelSurface,
        _seat: WlSeat,
        _serial: Serial,
        _edges: smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge,
    ) {
        // TODO: begin interactive resize
        tracing::debug!("Interactive resize requested");
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {
        // Popup grab — needed for dropdown menus
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let window = self.window_for_surface(surface.wl_surface());
        if let Some(window) = window {
            let ptr_id = window_ptr_id(&window);
            if let Some(&id) = self.window_ids.get(&ptr_id) {
                self.windows.remove(&id);
                self.window_ids.remove(&ptr_id);
                self.space.unmap_elem(&window);
                tracing::info!(window_id = id, "Toplevel window closed");
            }
        }
    }
}

impl JarvisCompositor {
    fn window_for_surface(
        &self,
        surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    ) -> Option<Window> {
        self.space
            .elements()
            .find(|w| {
                w.toplevel()
                    .map(|t| t.wl_surface() == surface)
                    .unwrap_or(false)
            })
            .cloned()
    }

    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let output = self.space.outputs().next().cloned();
        if let Some(output) = output {
            let output_geo = self.space.output_geometry(&output).unwrap_or_default();
            let _ = popup.with_pending_state(|state| {
                state.geometry = output_geo;
            });
        }
    }

    fn next_window_position(&self) -> smithay::utils::Point<i32, smithay::utils::Logical> {
        // Cascade new windows diagonally from top-left
        let offset = (self.windows.len() as i32) * 32;
        (32 + offset, 32 + offset).into()
    }
}

fn window_ptr_id(window: &Window) -> usize {
    window as *const Window as usize
}

delegate_xdg_shell!(JarvisCompositor);
