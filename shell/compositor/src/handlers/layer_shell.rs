use crate::state::JarvisCompositor;
use smithay::{
    delegate_layer_shell,
    desktop::{layer_map_for_output, LayerSurface},
    output::Output,
    wayland::shell::wlr_layer::{Layer, WlrLayerShellHandler, WlrLayerShellState},
};

/// Handles the wlr-layer-shell protocol.
///
/// jarvis-shell (Qt6) uses this to anchor the taskbar, launcher, and overlays
/// to screen edges. These surfaces sit above or below normal windows depending
/// on their declared layer.
impl WlrLayerShellHandler for JarvisCompositor {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: LayerSurface,
        output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        // Attach the layer surface to the first available output
        let output = output
            .as_ref()
            .and_then(|o| self.space.outputs().find(|out| out.owns(o)))
            .cloned()
            .or_else(|| self.space.outputs().next().cloned());

        let Some(output) = output else {
            tracing::warn!(namespace, "Layer surface mapped with no available output");
            surface.send_close();
            return;
        };

        tracing::info!(namespace, layer = ?_layer, "New layer surface from jarvis-shell");

        let mut map = layer_map_for_output(&output);
        map.map_layer(&surface).ok();
    }

    fn layer_destroyed(&mut self, surface: LayerSurface) {
        // Find and remove from all outputs
        for output in self.space.outputs().cloned().collect::<Vec<_>>() {
            let mut map = layer_map_for_output(&output);
            map.unmap_layer(&surface);
        }
        tracing::info!("Layer surface destroyed");
    }
}

delegate_layer_shell!(JarvisCompositor);
