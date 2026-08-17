use smithay::{
    delegate_fractional_scale, delegate_viewporter,
    reexports::wayland_server::protocol::wl_surface::WlSurface, wayland::compositor::with_states,
    wayland::fractional_scale::FractionalScaleHandler,
};

use crate::App;

impl FractionalScaleHandler for App {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        let scale = self
            .output_manager
            .outputs
            .first()
            .map(|o| o.current_scale().fractional_scale())
            .unwrap_or(1.0);

        with_states(&surface, |states| {
            smithay::wayland::fractional_scale::with_fractional_scale(states, |fractional_scale| {
                fractional_scale.set_preferred_scale(scale);
            });
        });
    }
}

delegate_fractional_scale!(App);
delegate_viewporter!(App);
