use smithay::utils::{Logical, Point};

use crate::state::{Rect, State, WindowId};

/// Target currently under the cursor/pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerFocusTarget {
    Window(WindowId),
    Background,
}

/// Active interactive drag operation (e.g. Super + Mouse drag).
#[derive(Debug, Clone, PartialEq)]
pub enum PointerDragMode {
    None,
    Move {
        window_id: WindowId,
        start_pointer: Point<f64, Logical>,
        initial_geom: Rect,
    },
    Resize {
        window_id: WindowId,
        start_pointer: Point<f64, Logical>,
        initial_geom: Rect,
    },
}

/// Tracks the pointer's logical coordinates, focus target, and active drag states.
#[derive(Debug, Clone)]
pub struct PointerState {
    pub location: Point<f64, Logical>,
    pub focus: PointerFocusTarget,
    pub drag: PointerDragMode,
}

impl Default for PointerState {
    fn default() -> Self {
        Self::new()
    }
}

impl PointerState {
    pub fn new() -> Self {
        Self {
            location: Point::from((0.0, 0.0)),
            focus: PointerFocusTarget::Background,
            drag: PointerDragMode::None,
        }
    }

    /// Set location directly.
    pub fn set_location(&mut self, location: Point<f64, Logical>) {
        self.location = location;
    }

    /// Update location by delta and clamp to bounding box.
    pub fn update_location(&mut self, delta: Point<f64, Logical>, bounds: Rect) {
        let new_x = (self.location.x + delta.x)
            .clamp(bounds.x as f64, (bounds.x + bounds.width as i32) as f64);
        let new_y = (self.location.y + delta.y)
            .clamp(bounds.y as f64, (bounds.y + bounds.height as i32) as f64);
        self.location = Point::from((new_x, new_y));
    }

    /// Find which window contains the pointer position on the active workspace.
    pub fn find_target_at_location(&self, state: &State) -> PointerFocusTarget {
        let ws = state.active_workspace();
        let px = self.location.x as i32;
        let py = self.location.y as i32;

        // Iterate over workspace windows in reverse (top-most first)
        for &win_id in ws.windows.iter().rev() {
            if let Some(win) = state.windows.get(&win_id) {
                let r = &win.geometry;
                if px >= r.x && px < r.x + r.width as i32 && py >= r.y && py < r.y + r.height as i32
                {
                    return PointerFocusTarget::Window(win_id);
                }
            }
        }

        PointerFocusTarget::Background
    }

    /// Begin interactive move drag on target window.
    pub fn start_drag_move(&mut self, window_id: WindowId, initial_geom: Rect) {
        self.drag = PointerDragMode::Move {
            window_id,
            start_pointer: self.location,
            initial_geom,
        };
    }

    /// Begin interactive resize drag on target window.
    pub fn start_drag_resize(&mut self, window_id: WindowId, initial_geom: Rect) {
        self.drag = PointerDragMode::Resize {
            window_id,
            start_pointer: self.location,
            initial_geom,
        };
    }

    /// Update geometry of dragged window if drag mode is active.
    pub fn update_drag(&self, state: &mut State) {
        match &self.drag {
            PointerDragMode::Move {
                window_id,
                start_pointer,
                initial_geom,
            } => {
                if let Some(win) = state.windows.get_mut(window_id) {
                    let dx = (self.location.x - start_pointer.x) as i32;
                    let dy = (self.location.y - start_pointer.y) as i32;
                    win.geometry.x = initial_geom.x + dx;
                    win.geometry.y = initial_geom.y + dy;
                    win.floating = true;
                }
            }
            PointerDragMode::Resize {
                window_id,
                start_pointer,
                initial_geom,
            } => {
                if let Some(win) = state.windows.get_mut(window_id) {
                    let dx = (self.location.x - start_pointer.x) as i32;
                    let dy = (self.location.y - start_pointer.y) as i32;
                    win.geometry.width = (initial_geom.width as i32 + dx).max(100) as u32;
                    win.geometry.height = (initial_geom.height as i32 + dy).max(100) as u32;
                    win.floating = true;
                }
            }
            PointerDragMode::None => {}
        }
    }

    /// End active drag.
    pub fn end_drag(&mut self) {
        self.drag = PointerDragMode::None;
    }
}
