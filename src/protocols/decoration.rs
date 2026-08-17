use smithay::{
    delegate_xdg_decoration,
    reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode,
    wayland::shell::xdg::{decoration::XdgDecorationHandler, ToplevelSurface},
};
use tracing::{debug, info};

use crate::App;

fn find_window_metadata(
    app: &App,
    surface: &ToplevelSurface,
) -> (Option<crate::state::WindowId>, String, String) {
    if let Some((&win_id, _)) = app
        .surfaces
        .iter()
        .find(|(_, s)| s.wl_surface() == surface.wl_surface())
    {
        if let Some(win) = app.state.windows.get(&win_id) {
            return (
                Some(win_id),
                win.app_id.clone().unwrap_or_else(|| "unknown".into()),
                win.title.clone().unwrap_or_else(|| "unknown".into()),
            );
        }
        return (Some(win_id), "unknown".into(), "unknown".into());
    }
    (None, "unmapped".into(), "unmapped".into())
}

impl XdgDecorationHandler for App {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        let (win_id, app_id, title) = find_window_metadata(self, &toplevel);
        info!(
            window_id = ?win_id,
            app_id = %app_id,
            title = %title,
            mode = ?Mode::ServerSide,
            "xdg_decoration: client initiated decoration negotiation, configuring ServerSide"
        );
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ServerSide);
        });
        let serial = toplevel.send_configure();
        debug!(
            window_id = ?win_id,
            serial = ?serial,
            "xdg_decoration: sent configure serial for ServerSide mode"
        );
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: Mode) {
        let (win_id, app_id, title) = find_window_metadata(self, &toplevel);
        info!(
            window_id = ?win_id,
            app_id = %app_id,
            title = %title,
            requested_mode = ?mode,
            "xdg_decoration: client requested decoration mode"
        );
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(mode);
        });
        let serial = toplevel.send_configure();
        debug!(
            window_id = ?win_id,
            serial = ?serial,
            mode = ?mode,
            "xdg_decoration: sent configure serial for requested mode"
        );
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        let (win_id, app_id, title) = find_window_metadata(self, &toplevel);
        info!(
            window_id = ?win_id,
            app_id = %app_id,
            title = %title,
            default_mode = ?Mode::ServerSide,
            "xdg_decoration: client unset decoration mode preference, defaulting to ServerSide"
        );
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ServerSide);
        });
        let serial = toplevel.send_configure();
        debug!(
            window_id = ?win_id,
            serial = ?serial,
            "xdg_decoration: sent configure serial for reset ServerSide mode"
        );
    }
}

delegate_xdg_decoration!(App);
