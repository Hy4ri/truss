use std::collections::HashMap;

use smithay::backend::renderer::Color32F;
use smithay::desktop::space::Space;
use smithay::desktop::Window as SmithayWindow;
use smithay::utils::{Logical, Point};
use smithay::wayland::shell::xdg::ToplevelSurface;

use crate::state::{State, WindowId};

/// Background color for the root desktop canvas (truss dark aesthetic).
pub const DESKTOP_BG_COLOR: Color32F = Color32F::new(0.08, 0.08, 0.10, 1.0);

/// Manages desktop window positioning and compositing space.
pub struct RenderManager {
    pub space: Space<SmithayWindow>,
}

impl Default for RenderManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderManager {
    pub fn new() -> Self {
        Self {
            space: Space::default(),
        }
    }

    /// Sync internal Smithay Space with truss State window geometries and active workspace.
    pub fn sync_windows(&mut self, state: &State, surfaces: &HashMap<WindowId, ToplevelSurface>) {
        let active_ws = state.active_workspace();

        // 1. Remove windows no longer in active workspace
        let current_windows = self.space.elements().cloned().collect::<Vec<_>>();
        for swin in current_windows {
            let matches_active = surfaces.iter().any(|(win_id, toplevel)| {
                active_ws.windows.contains(win_id)
                    && swin
                        .toplevel()
                        .map(|t| t.wl_surface() == toplevel.wl_surface())
                        .unwrap_or(false)
            });

            if !matches_active {
                self.space.unmap_elem(&swin);
            }
        }

        // 2. Map and position active workspace windows according to state geometry
        for &win_id in &active_ws.windows {
            if let (Some(win_state), Some(toplevel)) =
                (state.windows.get(&win_id), surfaces.get(&win_id))
            {
                let smithay_window = SmithayWindow::new_wayland_window(toplevel.clone());
                let loc = Point::<i32, Logical>::from((win_state.geometry.x, win_state.geometry.y));

                let already_mapped = self.space.elements().any(|w| {
                    w.toplevel()
                        .map(|t| t.wl_surface() == toplevel.wl_surface())
                        .unwrap_or(false)
                });

                if !already_mapped {
                    self.space.map_element(smithay_window, loc, true);
                } else {
                    self.space.map_element(smithay_window, loc, false);
                }
            }
        }
    }
}
