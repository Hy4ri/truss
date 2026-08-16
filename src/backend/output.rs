use smithay::output::{Mode as OutputMode, Output, PhysicalProperties, Subpixel};
use smithay::utils::{Point, Size};

use crate::state::Rect;

/// Description of a display output's physical and layout arrangement.
#[derive(Debug, Clone)]
pub struct OutputInfo {
    pub name: String,
    pub geometry: Rect,
    pub scale: f64,
    pub refresh: i32,
}

/// Manages physical or virtual display outputs with multi-monitor arrangement.
#[derive(Debug)]
pub struct OutputManager {
    pub outputs: Vec<Output>,
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputManager {
    pub fn new() -> Self {
        Self {
            outputs: Vec::new(),
        }
    }

    /// Creates and configures an output device positioned at `position` with resolution `size`.
    pub fn create_output(
        &mut self,
        name: &str,
        position: Point<i32, smithay::utils::Logical>,
        size: Size<i32, smithay::utils::Logical>,
        refresh_mhz: i32,
    ) -> Output {
        let output = Output::new(
            name.to_string(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "truss".to_string(),
                model: "display".to_string(),
            },
        );

        let mode = OutputMode {
            size: (size.w, size.h).into(),
            refresh: refresh_mhz,
        };

        output.change_current_state(Some(mode), None, None, Some(position));
        output.set_preferred(mode);

        self.outputs.push(output.clone());
        output
    }

    /// Creates and configures a default output device (e.g. 1920x1080 @ 60Hz at 0,0).
    pub fn create_default_output(
        &mut self,
        name: &str,
        size: Size<i32, smithay::utils::Logical>,
    ) -> Output {
        self.create_output(name, Point::from((0, 0)), size, 60_000)
    }

    /// Find an output by its connector / display name.
    pub fn find_output_by_name(&self, name: &str) -> Option<&Output> {
        self.outputs.iter().find(|o| o.name() == name)
    }

    /// Remove an output by name (e.g. unplugged display).
    pub fn remove_output(&mut self, name: &str) -> bool {
        let prev_len = self.outputs.len();
        self.outputs.retain(|o| o.name() != name);
        self.outputs.len() < prev_len
    }

    /// Total bounding box enclosing all active outputs.
    pub fn total_bounding_box(&self) -> Rect {
        if self.outputs.is_empty() {
            return Rect::new(0, 0, 1920, 1080);
        }

        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for output in &self.outputs {
            let pos = output.current_location();
            let size = output
                .current_mode()
                .map(|m| (m.size.w, m.size.h))
                .unwrap_or((1920, 1080));

            min_x = min_x.min(pos.x);
            min_y = min_y.min(pos.y);
            max_x = max_x.max(pos.x + size.0);
            max_y = max_y.max(pos.y + size.1);
        }

        Rect::new(min_x, min_y, (max_x - min_x) as u32, (max_y - min_y) as u32)
    }

    /// Gets the primary output's usable geometry.
    pub fn primary_usable_area(&self) -> Rect {
        if let Some(output) = self.outputs.first() {
            let pos = output.current_location();
            if let Some(mode) = output.current_mode() {
                return Rect::new(pos.x, pos.y, mode.size.w as u32, mode.size.h as u32);
            }
        }
        Rect::new(0, 0, 1920, 1080)
    }

    /// Retrieve metadata information for all managed outputs.
    pub fn output_infos(&self) -> Vec<OutputInfo> {
        self.outputs
            .iter()
            .map(|o| {
                let pos = o.current_location();
                let (w, h, refresh) = o
                    .current_mode()
                    .map(|m| (m.size.w as u32, m.size.h as u32, m.refresh))
                    .unwrap_or((1920, 1080, 60_000));
                let scale = o.current_scale().fractional_scale();

                OutputInfo {
                    name: o.name(),
                    geometry: Rect::new(pos.x, pos.y, w, h),
                    scale,
                    refresh,
                }
            })
            .collect()
    }
}
