use super::{Layout, LayoutConfig};
use crate::state::{Rect, WindowId};

/// Monocle / Fullscreen layout.
///
/// Every window is given the full usable area minus gaps (or stacked exactly on top of each other).
#[derive(Debug, Default, Clone, Copy)]
pub struct MonocleLayout;

impl Layout for MonocleLayout {
    fn name(&self) -> &'static str {
        "monocle"
    }

    fn arrange(
        &self,
        windows: &[WindowId],
        usable_area: Rect,
        config: &LayoutConfig,
    ) -> Vec<(WindowId, Rect)> {
        if windows.is_empty() {
            return Vec::new();
        }

        let gap = config.gap as i32;
        let width = (usable_area.width as i32 - 2 * gap).max(1) as u32;
        let height = (usable_area.height as i32 - 2 * gap).max(1) as u32;
        let rect = Rect::new(usable_area.x + gap, usable_area.y + gap, width, height);

        windows.iter().map(|&id| (id, rect)).collect()
    }
}
