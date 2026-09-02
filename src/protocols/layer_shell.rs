use smithay::{
    delegate_layer_shell,
    desktop::{layer_map_for_output, LayerSurface as DesktopLayerSurface},
    wayland::shell::wlr_layer::{
        Layer, LayerSurface as WlrLayerSurface, WlrLayerShellHandler, WlrLayerShellState,
    },
};
use tracing::{info, warn};

use crate::App;

impl WlrLayerShellHandler for App {
    fn ack_configure(
        &mut self,
        surface: smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
        _configure: smithay::wayland::shell::wlr_layer::LayerSurfaceConfigure,
    ) {
        self.needs_redraw = true;

        use smithay::wayland::compositor::with_states;
        use smithay::wayland::shell::wlr_layer::{KeyboardInteractivity, LayerSurfaceCachedState};

        let wants_keyboard = with_states(&surface, |states| {
            let mut cached = states.cached_state.get::<LayerSurfaceCachedState>();
            match cached.current().keyboard_interactivity {
                KeyboardInteractivity::Exclusive | KeyboardInteractivity::OnDemand => true,
                KeyboardInteractivity::None => match cached.pending().keyboard_interactivity {
                    KeyboardInteractivity::Exclusive | KeyboardInteractivity::OnDemand => true,
                    KeyboardInteractivity::None => false,
                },
            }
        });

        if wants_keyboard {
            if let Some(keyboard) = self.seat.get_keyboard() {
                let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                keyboard.set_focus(self, Some(surface), serial);
            }
        }
    }

    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>,
        layer: Layer,
        namespace: String,
    ) {
        info!(
            "wlr_layer_shell: new layer surface namespace='{}', layer={:?}",
            namespace, layer
        );

        let target_output = output
            .as_ref()
            .and_then(|o| self.output_manager.outputs.iter().find(|out| out.owns(o)))
            .or_else(|| self.output_manager.outputs.first())
            .cloned();

        if let Some(ref out) = target_output {
            let desktop_surface = DesktopLayerSurface::new(surface.clone(), namespace);
            {
                let mut layer_map = layer_map_for_output(out);
                if let Err(e) = layer_map.map_layer(&desktop_surface) {
                    warn!("Failed to map layer surface: {e}");
                }
                let _ = layer_map.arrange();

                surface.with_pending_state(|state| {
                    if state.size.is_none() {
                        // At map time the client hasn't committed a size yet, so
                        // layer_geometry() is a zero-sized rect (e.g. 0x0). That is
                        // Some(0x0), so the fallback below would NOT trigger and we
                        // would configure the client with width=0 height=0. Clients
                        // like fuzzel reject a 0x0 configure, never commit, and stay
                        // invisible. Treat both no geometry AND zero geometry as
                        // unknown and hand the client the output's full size so it
                        // gets a real, non-zero configure to ack against.
                        let geo = layer_map.layer_geometry(&desktop_surface);
                        let size = match geo {
                            Some(g) if g.size.w > 0 && g.size.h > 0 => (g.size.w, g.size.h).into(),
                            _ => out
                                .current_mode()
                                .map(|m| (m.size.w, m.size.h).into())
                                .unwrap_or((1920, 1080).into()),
                        };
                        state.size = Some(size);
                    }
                });
            } // Explicitly drop layer_map before refresh_layout_and_space to avoid mutex self-deadlock

            surface.send_configure();

            self.refresh_layout_and_space();
            self.needs_redraw = true;
        }
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        info!("wlr_layer_shell: layer surface destroyed");
        for output in &self.output_manager.outputs {
            let mut layer_map = layer_map_for_output(output);
            let to_remove = layer_map
                .layers()
                .find(|l| l.wl_surface() == surface.wl_surface())
                .cloned();
            if let Some(l) = to_remove {
                layer_map.unmap_layer(&l);
            }
            let _ = layer_map.arrange();
        }
        self.refresh_layout_and_space();
        self.needs_redraw = true;

        // Restore keyboard focus to the active workspace's focused window
        let focused_id = self.state.active_workspace().focused_window;
        self.set_focused_window(focused_id);
    }
}

delegate_layer_shell!(App);
