use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor,
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorHandler, CompositorState},
    },
};
use tracing::trace;
use wayland_server::{
    backend::{ClientData, ClientId, DisconnectReason},
    protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface},
    Client,
};

use crate::App;

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {
        trace!("Client initialized");
    }

    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {
        trace!("Client disconnected");
    }
}

impl AsMut<CompositorState> for App {
    fn as_mut(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }
}

impl CompositorHandler for App {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        self.popups.commit(surface);

        if let Some(pending_id) = self.pending_focus_window {
            if self
                .surfaces
                .get(&pending_id)
                .map(|toplevel| toplevel.wl_surface() == surface)
                .unwrap_or(false)
            {
                self.pending_focus_window = None;
                self.set_focused_window(Some(pending_id));
            }
        }

        // Notify transaction manager of committed surface
        if let Some((&win_id, _)) = self
            .surfaces
            .iter()
            .find(|(_, s)| s.wl_surface() == surface)
        {
            self.transaction_manager.on_surface_commit(win_id);
            self.needs_redraw = true;
        } else if self.output_manager.outputs.iter().any(|output| {
            use smithay::desktop::layer_map_for_output;
            layer_map_for_output(output)
                .layers()
                .any(|layer| layer.wl_surface() == surface)
        }) {
            // Layer-shell commit (bar, notification daemon). These are not
            // tracked toplevels, so without this the bar's own updates
            // (clock ticks!) never repaint on the render-on-demand TTY
            // backend until some unrelated event flags a redraw.
            self.needs_redraw = true;
            // Re-arrange every output's layer map: the client just committed
            // its real size, but layer geometry was last computed at map time
            // (before the buffer existed). Without this, layer_geometry()
            // stays zero and render.rs skips the surface -> invisible launcher.
            use smithay::desktop::layer_map_for_output;
            for output in &self.output_manager.outputs {
                let mut lm = layer_map_for_output(output);
                let _ = lm.arrange();
                for layer in lm.layers() {
                    if layer.wl_surface() == surface {
                        layer.layer_surface().send_configure();
                    }
                }
                lm.arrange();
            }

            // Grant keyboard focus to layer surfaces that request keyboard interactivity (e.g. launchers)
            use smithay::wayland::compositor::with_states;
            use smithay::wayland::shell::wlr_layer::{
                KeyboardInteractivity, LayerSurfaceCachedState,
            };

            let wants_keyboard = with_states(surface, |states| {
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
                    keyboard.set_focus(self, Some(surface.clone()), serial);
                }
            }
        }
    }
}

impl BufferHandler for App {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

delegate_compositor!(App);
