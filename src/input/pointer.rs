use smithay::utils::{Logical, Point};

use crate::state::{Rect, State, WindowId};

/// Target currently under the cursor/pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerFocusTarget {
    Window(WindowId),
    Background,
}

/// Tracks the pointer's logical coordinates and target under cursor.
#[derive(Debug, Clone)]
pub struct PointerState {
    pub location: Point<f64, Logical>,
    pub focus: PointerFocusTarget,
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
        }
    }

    /// Set location directly.
    pub fn set_location(&mut self, location: Point<f64, Logical>) {
        self.location = location;
    }

    /// Update location by delta.
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
}
