use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::surface::{
    render_elements_from_surface_tree, WaylandSurfaceRenderElement,
};
use smithay::backend::renderer::element::utils::CropRenderElement;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::desktop::layer_map_for_output;
use smithay::input::pointer::CursorImageStatus;
use smithay::render_elements;
use smithay::wayland::shell::wlr_layer::Layer as WlrLayer;

use crate::backend::cursor::CursorManager;
use crate::App;

// Unified render element enum that can hold both Wayland surface elements
// and memory-buffer-backed cursor elements.
render_elements! {
    pub TrussRenderElement<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    CroppedSurface=CropRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>,
    Cursor=MemoryRenderBufferRenderElement<GlesRenderer>,
}

/// Extract all render elements (cursor, overlay, top, popups, toplevel windows, bottom, background)
/// for rendering to the active output framebuffer in FRONT-TO-BACK (top-to-bottom) order
/// as required by Smithay's `draw_render_elements` and `OutputDamageTracker`.
pub fn collect_render_elements(
    app: &App,
    renderer: &mut GlesRenderer,
    cursor_manager: &mut CursorManager,
) -> Vec<TrussRenderElement> {
    let mut elements = Vec::new();

    // 1. Cursor (top-most layer, rendered on top of everything)
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

    // 2. Layer Shell Surfaces: Overlay & Top layers
    for output in &app.output_manager.outputs {
        let layer_map = layer_map_for_output(output);

        for surface in layer_map
            .layers_on(WlrLayer::Overlay)
            .chain(layer_map.layers_on(WlrLayer::Top))
        {
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

    // 3. Normal toplevel windows and popups in layout order. The workspace
    // list is the source of truth for master/stack ordering; iterating all XDG
    // surfaces here could render an unmapped or stale surface at (0, 0).
    for &window_id in &app.state.active_workspace().windows {
        let (Some(surface), Some(window)) = (
            app.surfaces.get(&window_id),
            app.state.windows.get(&window_id),
        ) else {
            continue;
        };

        let win_geom = (window.geometry.x, window.geometry.y);

        // Render associated popups first (above parent windows)
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

        let win_elements = render_elements_from_surface_tree(
            renderer,
            surface.wl_surface(),
            win_geom,
            1.0,
            1.0,
            Kind::Unspecified,
        );
        // A client is allowed to keep its old buffer until it acknowledges the
        // resize configure. Crop it to the assigned tile meanwhile, otherwise
        // an older master window can visually remain full-screen and cover the
        // stack after another application opens.
        let tile = smithay::utils::Rectangle::new(
            win_geom.into(),
            (window.geometry.width as i32, window.geometry.height as i32).into(),
        );
        elements.extend(
            win_elements
                .into_iter()
                .filter_map(|element| CropRenderElement::from_element(element, 1.0, tile))
                .map(TrussRenderElement::CroppedSurface),
        );
    }

    // 4. Layer Shell Surfaces: Bottom & Background layers
    for output in &app.output_manager.outputs {
        let layer_map = layer_map_for_output(output);

        for surface in layer_map
            .layers_on(WlrLayer::Bottom)
            .chain(layer_map.layers_on(WlrLayer::Background))
        {
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

    elements
}
