use super::{Layout, LayoutConfig};
use crate::state::{Rect, WindowId};

/// Master-and-stack dynamic tiling layout.
///
/// If 1 window: spans the full usable area (with gaps).
/// If >= 2 windows: Master area on the left (controlled by `master_ratio`),
/// and remaining windows vertically stacked on the right.
#[derive(Debug, Default, Clone, Copy)]
pub struct MasterLayout;

impl Layout for MasterLayout {
    fn name(&self) -> &'static str {
        "master"
    }

    fn arrange(
        &self,
        windows: &[WindowId],
        usable_area: Rect,
        config: &LayoutConfig,
    ) -> Vec<(WindowId, Rect)> {
        let n = windows.len();
        if n == 0 {
            return Vec::new();
        }

        let gap = config.gap as i32;
        let master_ratio = config.master_ratio.clamp(0.1, 0.9);

        // Case 1: Single window gets the whole usable area minus outer gap
        if n == 1 {
            let width = (usable_area.width as i32 - 2 * gap).max(1) as u32;
            let height = (usable_area.height as i32 - 2 * gap).max(1) as u32;
            return vec![(
                windows[0],
                Rect::new(usable_area.x + gap, usable_area.y + gap, width, height),
            )];
        }

        // Case 2: Master on the left, stack on the right
        // Total available inner width excluding outer gaps (left, middle, right)
        let total_w = usable_area.width as i32 - 3 * gap;
        if total_w <= 0 {
            // Degenerate small display
            return windows
                .iter()
                .map(|&id| (id, Rect::new(usable_area.x, usable_area.y, 1, 1)))
                .collect();
        }

        let master_w = ((total_w as f32) * master_ratio).round() as i32;
        let stack_w = total_w - master_w;

        let master_rect = Rect::new(
            usable_area.x + gap,
            usable_area.y + gap,
            master_w.max(1) as u32,
            (usable_area.height as i32 - 2 * gap).max(1) as u32,
        );

        let stack_count = (n - 1) as i32;
        let total_stack_h = usable_area.height as i32 - (stack_count + 1) * gap;
        let single_stack_h = (total_stack_h / stack_count).max(1);
        let stack_x = usable_area.x + gap + master_w + gap;

        let mut results = Vec::with_capacity(n);
        results.push((windows[0], master_rect));

        for (i, &win_id) in windows[1..].iter().enumerate() {
            let idx = i as i32;
            let stack_y = usable_area.y + gap + idx * (single_stack_h + gap);

            // Last window gets any remainder pixel height to prevent gaps due to integer division
            let h = if idx == stack_count - 1 {
                (usable_area.y + usable_area.height as i32 - gap - stack_y).max(1) as u32
            } else {
                single_stack_h.max(1) as u32
            };

            results.push((
                win_id,
                Rect::new(stack_x, stack_y, stack_w.max(1) as u32, h),
            ));
        }

        results
    }
}
