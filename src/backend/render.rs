use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
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

// Unified render element enum that can hold Wayland surface elements,
// memory-buffer-backed cursor elements, and solid color borders.
render_elements! {
    pub TrussRenderElement<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Cursor=MemoryRenderBufferRenderElement<GlesRenderer>,
    Solid=SolidColorRenderElement,
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

    // Helper closure to render a window and its popups and borders
    let render_window_tree = |elements: &mut Vec<TrussRenderElement>,
                              renderer: &mut GlesRenderer,
                              window_id: crate::state::WindowId| {
        let (Some(surface), Some(window)) = (
            app.surfaces.get(&window_id),
            app.state.windows.get(&window_id),
        ) else {
            return;
        };

        let win_geom = (window.geometry.x, window.geometry.y);

        // Render associated popups first (above parent window)
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

        // Window borders for active and inactive windows (fullscreen windows omit borders)
        if app.border_config.width > 0 && !window.fullscreen {
            let b = app.border_config.width as i32;
            let (x, y) = win_geom;
            let (w, h) = (window.geometry.width as i32, window.geometry.height as i32);
            let is_active = Some(window_id) == app.state.active_workspace().focused_window;
            let border_color = if is_active {
                app.border_config.active_color
            } else {
                app.border_config.inactive_color
            };

            let top_buf = SolidColorBuffer::new((w + 2 * b, b), border_color);
            elements.push(TrussRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &top_buf,
                    (x - b, y - b),
                    1.0,
                    1.0,
                    Kind::Unspecified,
                ),
            ));

            let bottom_buf = SolidColorBuffer::new((w + 2 * b, b), border_color);
            elements.push(TrussRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &bottom_buf,
                    (x - b, y + h),
                    1.0,
                    1.0,
                    Kind::Unspecified,
                ),
            ));

            let left_buf = SolidColorBuffer::new((b, h), border_color);
            elements.push(TrussRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &left_buf,
                    (x - b, y),
                    1.0,
                    1.0,
                    Kind::Unspecified,
                ),
            ));

            let right_buf = SolidColorBuffer::new((b, h), border_color);
            elements.push(TrussRenderElement::Solid(
                SolidColorRenderElement::from_buffer(
                    &right_buf,
                    (x + w, y),
                    1.0,
                    1.0,
                    Kind::Unspecified,
                ),
            ));
        }
    };

    let active_ws = app.state.active_workspace();

    // 2. Fullscreen Windows (TOP-MOST: max on top, even covers Overlay & Top layers/bar)
    for &window_id in active_ws.windows.iter().rev() {
        if let Some(win) = app.state.windows.get(&window_id) {
            if win.fullscreen {
                render_window_tree(&mut elements, renderer, window_id);
            }
        }
    }

    // 3. Layer Shell Surfaces: Overlay & Top layers (e.g. Waybar, notifications, launchers)
    for output in &app.output_manager.outputs {
        let layer_map = layer_map_for_output(output);

        for surface in layer_map
            .layers_on(WlrLayer::Overlay)
            .chain(layer_map.layers_on(WlrLayer::Top))
        {
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
            elements.extend(layer_elements.into_iter().map(TrussRenderElement::Surface));
        }
    }

    // Direct fallback: render all active layer surfaces registered in layer_shell_state
    for layer_surface in app.layer_shell_state.layer_surfaces() {
        let wl_surf = layer_surface.wl_surface();
        let already_rendered = app.output_manager.outputs.iter().any(|o| {
            layer_map_for_output(o)
                .layers()
                .any(|l| l.wl_surface() == wl_surf)
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

    // 4. Floating Windows (Always on top of tiled windows)
    for &window_id in active_ws.windows.iter().rev() {
        if let Some(win) = app.state.windows.get(&window_id) {
            if win.floating && !win.fullscreen {
                render_window_tree(&mut elements, renderer, window_id);
            }
        }
    }

    // 5. Tiled Windows (In layout master/stack order)
    for &window_id in &active_ws.windows {
        if let Some(win) = app.state.windows.get(&window_id) {
            if !win.floating && !win.fullscreen {
                render_window_tree(&mut elements, renderer, window_id);
            }
        }
    }

    // 4. Layer Shell Surfaces: Bottom & Background layers
    for output in &app.output_manager.outputs {
        let layer_map = layer_map_for_output(output);

        for surface in layer_map
            .layers_on(WlrLayer::Bottom)
            .chain(layer_map.layers_on(WlrLayer::Background))
        {
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
            elements.extend(layer_elements.into_iter().map(TrussRenderElement::Surface));
        }
    }

    elements
}
