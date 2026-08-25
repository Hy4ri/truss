use smithay::utils::IsAlive;
use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::surface::{
    render_elements_from_surface_tree, WaylandSurfaceRenderElement,
};
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
    for o in &app.output_manager.outputs {
        let lm = layer_map_for_output(o);
        for l in lm.layers() {
            tracing::info!("LUNA-LM-FOUND: layer ns={}, geom={:?}, alive={}", l.namespace(), lm.layer_geometry(l), l.alive());
        }
        tracing::info!("LUNA-RENDER-ENTRY: o ptr: {:p}, userdata ptr: {:p}, name: {}, layers: {}", o, o.user_data(), o.name(), lm.layers().count());
    }

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

        for surface in layer_map.layers() {
            let geo = layer_map.layer_geometry(surface);
            let loc = geo.map(|g| (g.loc.x, g.loc.y)).unwrap_or((0, 0));
            let layer_elements = render_elements_from_surface_tree(
                renderer,
                surface.wl_surface(),
                loc,
                1.0,
                1.0,
                Kind::Unspecified,
            );
            tracing::info!("LUNA-RENDER-LAYERS: found {} elements for layer surface {:?}", layer_elements.len(), surface.namespace());
            elements.extend(layer_elements.into_iter().map(TrussRenderElement::Surface));
        }
    }

    // Direct fallback: render all active layer surfaces registered in layer_shell_state
    for layer_surface in app.layer_shell_state.layer_surfaces() {
        let wl_surf = layer_surface.wl_surface();
        // Avoid duplicate rendering if already rendered via LayerMap
        let already_rendered = app.output_manager.outputs.iter().any(|o| {
            layer_map_for_output(o).layers().any(|l| l.wl_surface() == wl_surf)
        });
        if !already_rendered {
            let layer_elements = render_elements_from_surface_tree(
                renderer,
                wl_surf,
                (0, 0),
                1.0,
                1.0,
                Kind::Unspecified,
            );
            elements.extend(layer_elements.into_iter().map(TrussRenderElement::Surface));
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
        elements.extend(win_elements.into_iter().map(TrussRenderElement::Surface));
    }

    // 4. Layer Shell Surfaces: Bottom & Background layers
    for output in &app.output_manager.outputs {
        let layer_map = layer_map_for_output(output);

        for surface in std::iter::empty::<&smithay::desktop::LayerSurface>()
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
