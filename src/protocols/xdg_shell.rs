use smithay::{
    delegate_xdg_shell,
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::protocol::wl_seat::WlSeat,
    },
    utils::Serial,
    wayland::shell::xdg::{
        PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    },
};
use tracing::{info, warn};

use crate::{dispatch::Event, App};

impl XdgShellHandler for App {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        // Initial configure: tell client it is activated
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Activated);
        });
        surface.send_configure();

        // Create window in pure state
        let window_id = match self.state.create_window(None) {
            Ok(id) => id,
            Err(e) => {
                warn!("Failed to create window in state: {e}");
                return;
            }
        };

        // Extract metadata if available through pending/current state
        surface.with_pending_state(|state| {
            if let Some(app_id) = &state.app_id {
                if let Some(win) = self.state.windows.get_mut(&window_id) {
                    win.app_id = Some(app_id.clone());
                }
            }
            if let Some(title) = &state.title {
                if let Some(win) = self.state.windows.get_mut(&window_id) {
                    win.title = Some(title.clone());
                }
            }
        });

        // Map window ID to toplevel surface
        self.surfaces.insert(window_id, surface);

        info!(
            "xdg_toplevel mapped: {:?} on workspace {}",
            window_id, self.state.active_workspace_id
        );

        self.dispatcher.broadcast(&Event::WindowCreated {
            id: window_id,
            workspace_id: self.state.active_workspace_id,
        });
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {
        // Popups handling in polish milestone
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let mut target_id = None;
        for (&id, s) in &self.surfaces {
            if s.wl_surface() == surface.wl_surface() {
                target_id = Some(id);
                break;
            }
        }

        if let Some(id) = target_id {
            self.surfaces.remove(&id);
            let _ = self.state.remove_window(id);
            info!("xdg_toplevel unmapped: {:?}", id);
            self.dispatcher.broadcast(&Event::WindowDestroyed { id });
        }
    }
}

delegate_xdg_shell!(App);
