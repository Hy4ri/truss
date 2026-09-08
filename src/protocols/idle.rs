use smithay::delegate_idle_inhibit;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::idle_inhibit::IdleInhibitHandler;

use crate::App;

impl IdleInhibitHandler for App {
    fn inhibit(&mut self, surface: WlSurface) {
        tracing::info!("Idle inhibition requested for surface {:?}", surface);
        self.inhibited_surfaces.insert(surface);
    }

    fn uninhibit(&mut self, surface: WlSurface) {
        tracing::info!("Idle inhibition released for surface {:?}", surface);
        self.inhibited_surfaces.remove(&surface);
    }
}

delegate_idle_inhibit!(App);
