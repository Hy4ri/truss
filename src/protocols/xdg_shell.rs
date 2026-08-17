use smithay::{
    delegate_xdg_shell,
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::protocol::wl_seat::WlSeat,
    },
    utils::Serial,
    wayland::{
        compositor,
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            XdgToplevelSurfaceData,
        },
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

        // Extract metadata if available through compositor states
        compositor::with_states(surface.wl_surface(), |states| {
            if let Some(data) = states.data_map.get::<XdgToplevelSurfaceData>() {
                if let Ok(guard) = data.lock() {
                    if let Some(win) = self.state.windows.get_mut(&window_id) {
                        win.app_id = guard.app_id.clone();
                        win.title = guard.title.clone();
                    }
                }
            }
        });

        // Map window ID to toplevel surface
        self.surfaces.insert(window_id, surface);

        // Evaluate and apply declarative window rules
        if let Some(win) = self.state.windows.get_mut(&window_id) {
            self.window_rules.evaluate_and_apply(win);
        }

        // Focus new toplevel window by default
        self.set_focused_window(Some(window_id));

        info!(
            "xdg_toplevel mapped: {:?} on workspace {}",
            window_id, self.state.active_workspace_id
        );

        self.dispatcher.broadcast(&Event::WindowCreated {
            id: window_id,
            workspace_id: self.state.active_workspace_id,
        });
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        let mut target_id = None;
        for (&id, s) in &self.surfaces {
            if s.wl_surface() == surface.wl_surface() {
                target_id = Some(id);
                break;
            }
        }

        if let Some(id) = target_id {
            compositor::with_states(surface.wl_surface(), |states| {
                if let Some(data) = states.data_map.get::<XdgToplevelSurfaceData>() {
                    if let Ok(guard) = data.lock() {
                        if let Some(win) = self.state.windows.get_mut(&id) {
                            win.app_id = guard.app_id.clone();
                        }
                    }
                }
            });
            if let Some(win) = self.state.windows.get_mut(&id) {
                self.window_rules.evaluate_and_apply(win);
            }
        }
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        let mut target_id = None;
        for (&id, s) in &self.surfaces {
            if s.wl_surface() == surface.wl_surface() {
                target_id = Some(id);
                break;
            }
        }

        if let Some(id) = target_id {
            compositor::with_states(surface.wl_surface(), |states| {
                if let Some(data) = states.data_map.get::<XdgToplevelSurfaceData>() {
                    if let Ok(guard) = data.lock() {
                        if let Some(win) = self.state.windows.get_mut(&id) {
                            win.title = guard.title.clone();
                        }
                    }
                }
            });
        }
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        // Track popup using PopupManager
        if let Err(err) = self.popups.track_popup(surface.into()) {
            warn!("Failed to track popup: {err}");
        }
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

            let next_focus = self.state.active_workspace().focused_window;
            self.set_focused_window(next_focus);
        }
    }
}

delegate_xdg_shell!(App);
