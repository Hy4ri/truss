use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::surface::{
    render_elements_from_surface_tree, WaylandSurfaceRenderElement,
};
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::desktop::layer_map_for_output;
use smithay::input::pointer::CursorImageStatus;
use smithay::render_elements;

use crate::backend::cursor::CursorManager;
use crate::App;

// Unified render element enum that can hold both Wayland surface elements
// and memory-buffer-backed cursor elements.
render_elements! {
    pub TrussRenderElement<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Cursor=MemoryRenderBufferRenderElement<GlesRenderer>,
}

/// Extract all render elements (layer-shell background, bottom, toplevel windows, top, overlay, cursor)
/// for rendering to the active output framebuffer in proper Wayland layer order.
pub fn collect_render_elements(
    app: &App,
    renderer: &mut GlesRenderer,
    cursor_manager: &mut CursorManager,
) -> Vec<TrussRenderElement> {
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
                elements.extend(layer_elements.into_iter().map(TrussRenderElement::Surface));
            }
        }
    }

    // 2. Normal Toplevel Windows & Popups
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
        elements.extend(win_elements.into_iter().map(TrussRenderElement::Surface));

        // Render associated popups (context menus, dropdowns, tooltips)
        for (popup, popup_loc) in
            smithay::desktop::PopupManager::popups_for_surface(surface.wl_surface())
        {
            let popup_abs_pos = (win_geom.0 + popup_loc.x, win_geom.1 + popup_loc.y);
            let popup_elements = render_elements_from_surface_tree(
                renderer,
                popup.wl_surface(),
                popup_abs_pos,
                1.0,
                1.0,
                Kind::Unspecified,
            );
            elements.extend(popup_elements.into_iter().map(TrussRenderElement::Surface));
        }
    }

    // 3. Cursor (rendered LAST = on top of everything)
    let pointer_loc = app.pointer_state.location;
    let cursor_pos = (pointer_loc.x as i32, pointer_loc.y as i32);

    match &app.cursor_status {
        CursorImageStatus::Hidden => {
            // No cursor to render
        }
        CursorImageStatus::Surface(wl_surface) => {
            // Client-provided cursor surface (e.g. text cursor, resize handles)
            let cursor_elements = render_elements_from_surface_tree(
                renderer,
                wl_surface,
                cursor_pos,
                1.0,
                1.0,
                Kind::Cursor,
            );
            elements.extend(cursor_elements.into_iter().map(TrussRenderElement::Surface));
        }
        CursorImageStatus::Named(_icon) => {
            // Use xcursor theme cursor or fallback
            if let Some(cursor_element) = cursor_manager.render_named_cursor(renderer, cursor_pos) {
                elements.push(TrussRenderElement::Cursor(cursor_element));
            }
        }
    }

    elements
}
