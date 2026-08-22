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
        }
    }
}

impl BufferHandler for App {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

delegate_compositor!(App);
