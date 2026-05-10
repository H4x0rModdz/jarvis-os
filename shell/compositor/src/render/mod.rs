use crate::state::JarvisCompositor;
use smithay::{
    backend::renderer::{
        damage::OutputDamageTracker,
        element::{
            surface::WaylandSurfaceRenderElement,
            utils::{
                constrain_render_elements, ConstrainAlign, ConstrainScaleBehavior,
                CropRenderElement, RelocateRenderElement, RescaleRenderElement,
            },
        },
        gles::GlesRenderer,
        ImportAll, ImportMem, Renderer,
    },
    desktop::{layer_map_for_output, Space, Window},
    output::Output,
    utils::{Physical, Rectangle, Scale, Transform},
};

/// Elements that can appear in a rendered frame.
smithay::backend::renderer::element::render_elements! {
    pub JarvisRenderElement<R> where R: ImportAll + ImportMem;
    Window=WaylandSurfaceRenderElement<R>,
}

/// Render a complete frame for the given output.
///
/// Returns true if any damage was present (frame needed rendering),
/// false if the screen was already up to date.
pub fn render_output<R>(
    renderer: &mut R,
    output: &Output,
    space: &Space<Window>,
    damage_tracker: &mut OutputDamageTracker,
) -> anyhow::Result<bool>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + 'static,
    R::Error: Send + Sync + 'static,
    JarvisRenderElement<R>: smithay::backend::renderer::element::Element,
{
    let scale = Scale::from(output.current_scale().fractional_scale());
    let output_geo = space
        .output_geometry(output)
        .unwrap_or_else(|| Rectangle::from_loc_and_size((0, 0), (1920, 1080)));

    // Collect all render elements: layer surfaces (bottom), windows, layer surfaces (top)
    let mut elements: Vec<JarvisRenderElement<R>> = Vec::new();

    // Draw background (solid dark color for now — wallpaper in Phase 2)
    // Elements are drawn back-to-front.

    // Windows from the space
    for window in space.elements() {
        if let Some(surface) = window.wl_surface() {
            let location = space.element_location(window).unwrap_or_default();
            let geo =
                smithay::utils::Rectangle::from_loc_and_size(location, window.geometry().size);

            let render_elements =
                smithay::backend::renderer::element::surface::render_elements_from_surface_tree(
                    renderer,
                    &surface,
                    (geo.loc.x, geo.loc.y),
                    scale,
                    1.0,
                    smithay::backend::renderer::element::Kind::Unspecified,
                );

            elements.extend(render_elements.into_iter().map(JarvisRenderElement::Window));
        }
    }

    // Layer shell surfaces (jarvis-shell taskbar and overlays)
    let layer_map = layer_map_for_output(output);
    for layer_surface in layer_map.layers() {
        if let Some(surface) = layer_surface.wl_surface() {
            let geo = layer_map.layer_geometry(layer_surface).unwrap_or_default();

            let render_elements =
                smithay::backend::renderer::element::surface::render_elements_from_surface_tree(
                    renderer,
                    &surface,
                    (geo.loc.x, geo.loc.y),
                    scale,
                    1.0,
                    smithay::backend::renderer::element::Kind::Unspecified,
                );

            elements.extend(render_elements.into_iter().map(JarvisRenderElement::Window));
        }
    }

    // Submit frame via damage tracker
    let (has_damage, _) =
        damage_tracker.render_output(renderer, 0, &elements, [0.05, 0.05, 0.08, 1.0])?;

    Ok(has_damage.is_some())
}
