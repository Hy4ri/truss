use smithay::output::{Mode as OutputMode, Output, PhysicalProperties, Subpixel};
use smithay::utils::{Point, Size};

use crate::state::Rect;

/// Manages physical or virtual display outputs.
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

    /// Creates and configures a default output device (e.g. 1920x1080 @ 60Hz).
    pub fn create_default_output(
        &mut self,
        name: &str,
        size: Size<i32, smithay::utils::Logical>,
    ) -> Output {
        let output = Output::new(
            name.to_string(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "truss".to_string(),
                model: "virtual".to_string(),
            },
        );

        let mode = OutputMode {
            size: (size.w, size.h).into(),
            refresh: 60_000,
        };

        output.change_current_state(Some(mode), None, None, Some(Point::from((0, 0))));
        output.set_preferred(mode);

        self.outputs.push(output.clone());
        output
    }

    /// Gets the primary output's usable geometry.
    pub fn primary_usable_area(&self) -> Rect {
        if let Some(output) = self.outputs.first() {
            if let Some(mode) = output.current_mode() {
                return Rect::new(0, 0, mode.size.w as u32, mode.size.h as u32);
            }
        }
        Rect::new(0, 0, 1920, 1080)
    }
}
