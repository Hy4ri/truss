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
            let mut layer_map = layer_map_for_output(out);
            if let Err(e) = layer_map.map_layer(&desktop_surface) {
                warn!("Failed to map layer surface: {e}");
            }
            let _ = layer_map.arrange();

            surface.with_pending_state(|state| {
                if state.size.is_none() {
                    let size = layer_map
                        .layer_geometry(&desktop_surface)
                        .map(|g| (g.size.w, g.size.h).into())
                        .unwrap_or_else(|| {
                            out.current_mode()
                                .map(|m| (m.size.w, m.size.h).into())
                                .unwrap_or((1920, 1080).into())
                        });
                    state.size = Some(size);
                }
            });
            surface.send_configure();

            self.refresh_layout_and_space();
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
    }
}

delegate_layer_shell!(App);
