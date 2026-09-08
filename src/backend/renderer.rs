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

        // 1. Remove windows no longer in active workspace (unless pinned or special workspace active)
        let current_windows = self.space.elements().cloned().collect::<Vec<_>>();
        for swin in current_windows {
            let matches_active = surfaces.iter().any(|(win_id, toplevel)| {
                let is_pinned = state.windows.get(win_id).map(|w| w.pinned).unwrap_or(false);
                let is_special = state.special_workspace_active
                    && state
                        .windows
                        .get(win_id)
                        .map(|w| w.workspace_id == crate::state::SPECIAL_WORKSPACE_ID)
                        .unwrap_or(false);
                (active_ws.windows.contains(win_id) || is_pinned || is_special)
                    && swin
                        .toplevel()
                        .map(|t| t.wl_surface() == toplevel.wl_surface())
                        .unwrap_or(false)
            });

            if !matches_active {
                self.space.unmap_elem(&swin);
            }
        }

        // 2. Map and position active workspace windows according to state geometry (including pinned/special)
        let mut visible_windows = active_ws.windows.clone();
        // If windows share a group_id, only render the active/focused one (or the first one)
        let active_win_id = active_ws.focused_window;
        let mut group_rendered = std::collections::HashSet::new();
        visible_windows.retain(|wid| {
            if let Some(win) = state.windows.get(wid) {
                if let Some(gid) = win.group_id {
                    if Some(*wid) == active_win_id {
                        group_rendered.insert(gid);
                        return true;
                    }
                    if group_rendered.contains(&gid) {
                        return false;
                    }
                    group_rendered.insert(gid);
                }
            }
            true
        });
        for (&wid, win) in &state.windows {
            if win.pinned && !visible_windows.contains(&wid) {
                visible_windows.push(wid);
            }
        }
        if state.special_workspace_active {
            if let Some(special_ws) = state.workspaces.get(&crate::state::SPECIAL_WORKSPACE_ID) {
                for &wid in &special_ws.windows {
                    if !visible_windows.contains(&wid) {
                        visible_windows.push(wid);
                    }
                }
            }
        }

        for &win_id in &visible_windows {
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
