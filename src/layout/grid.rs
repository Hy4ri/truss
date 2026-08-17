use crate::layout::{Layout, LayoutConfig};
use crate::state::{Rect, WindowId};

/// Grid layout: arranges N windows into an optimal R x C grid.
#[derive(Debug, Default, Clone, Copy)]
pub struct GridLayout;

impl Layout for GridLayout {
    fn name(&self) -> &'static str {
        "grid"
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
        let cols = (n as f32).sqrt().ceil() as usize;
        let rows = n.div_ceil(cols);

        let total_gap_w = gap * (cols as i32 + 1);
        let total_gap_h = gap * (rows as i32 + 1);

        let avail_w = (usable_area.width as i32 - total_gap_w).max(0);
        let avail_h = (usable_area.height as i32 - total_gap_h).max(0);

        let cell_w = (avail_w / cols as i32).max(1) as u32;
        let cell_h = (avail_h / rows as i32).max(1) as u32;

        let mut results = Vec::with_capacity(n);

        for (i, &win_id) in windows.iter().enumerate() {
            let r = i / cols;
            let c = i % cols;

            let x = usable_area.x + gap + (c as i32 * (cell_w as i32 + gap));
            let y = usable_area.y + gap + (r as i32 * (cell_h as i32 + gap));

            results.push((
                win_id,
                Rect {
                    x,
                    y,
                    width: cell_w,
                    height: cell_h,
                },
            ));
        }

        results
    }
}
