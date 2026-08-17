use smithay::backend::renderer::element::surface::{
    render_elements_from_surface_tree, WaylandSurfaceRenderElement,
};
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::desktop::layer_map_for_output;

use crate::App;

/// Extract all render elements (layer-shell background, bottom, toplevel windows, top, overlay)
/// for rendering to the active output framebuffer in proper Wayland layer order.
pub fn collect_render_elements(
    app: &App,
    renderer: &mut GlesRenderer,
) -> Vec<WaylandSurfaceRenderElement<GlesRenderer>> {
    let mut elements = Vec::new();

    // 1. Layer Shell Surfaces: non-popup background/bottom layers
    for output in &app.output_manager.outputs {
        let layer_map = layer_map_for_output(output);

        for surface in layer_map.layers() {
            if let Some(loc) = layer_map
                .layer_geometry(surface)
                .map(|g| (g.loc.x, g.loc.y))
            {
                let layer_elements = render_elements_from_surface_tree(
                    renderer,
                    surface.wl_surface(),
                    loc,
                    1.0,
                    1.0,
                    Kind::Unspecified,
                );
                elements.extend(layer_elements);
            }
        }
    }

    // 2. Normal Toplevel Windows
    for surface in app.xdg_shell_state.toplevel_surfaces() {
        let win_geom = app
            .surfaces
            .iter()
            .find(|(_, s)| s.wl_surface() == surface.wl_surface())
            .and_then(|(id, _)| app.state.windows.get(id))
            .map(|w| (w.geometry.x, w.geometry.y))
            .unwrap_or((0, 0));

        let win_elements = render_elements_from_surface_tree(
            renderer,
            surface.wl_surface(),
            win_geom,
            1.0,
            1.0,
            Kind::Unspecified,
        );
        elements.extend(win_elements);
    }

    elements
}
