use smithay::{
    delegate_layer_shell,
    wayland::shell::wlr_layer::{Layer, LayerSurface, WlrLayerShellHandler, WlrLayerShellState},
};
use tracing::info;

use crate::App;

impl WlrLayerShellHandler for App {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: LayerSurface,
        _output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>,
        layer: Layer,
        namespace: String,
    ) {
        info!(
            "wlr_layer_shell: new layer surface namespace='{}', layer={:?}",
            namespace, layer
        );

        let output = self.output_manager.outputs.first().cloned();

        if let Some(output) = output {
            surface.with_pending_state(|state| {
                let size = output
                    .current_mode()
                    .map(|m| (m.size.w, m.size.h).into())
                    .unwrap_or((1920, 1080).into());
                state.size = Some(size);
            });
            surface.send_configure();
        }
    }
}

delegate_layer_shell!(App);
