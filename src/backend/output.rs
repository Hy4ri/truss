use smithay::{
    output::{Mode as OutputMode, Output, PhysicalProperties, Subpixel},
    utils::{Point, Size},
};

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
    ///
    /// Note: the caller must advertise the returned [`Output`] to clients via
    /// [`Output::create_global`](smithay::output::Output::create_global) with the compositor's
    /// `DisplayHandle`, otherwise clients see no `wl_output` globals and refuse to start
    /// (e.g. terminal clients reporting "no monitors available").
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
                // Physical size in millimeters. A virtual/headless output gets a
                // nominal 96 DPI sizing so clients (kitty, etc.) compute sane DPI.
                size: (
                    (size.w as f64 * 25.4 / 96.0).round() as i32,
                    (size.h as f64 * 25.4 / 96.0).round() as i32,
                )
                    .into(),
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

    /// Gets the primary output's full display geometry (including areas covered by panels/bars).
    pub fn primary_full_area(&self) -> Rect {
        if let Some(output) = self.outputs.first() {
            if let Some(mode) = output.current_mode() {
                return Rect::new(0, 0, mode.size.w as u32, mode.size.h as u32);
            }
        }
        Rect::new(0, 0, 1920, 1080)
    }

    /// Gets the primary output's usable geometry (excluding panels/bars with exclusive zones).
    pub fn primary_usable_area(&self) -> Rect {
        if let Some(output) = self.outputs.first() {
            let layer_map = smithay::desktop::layer_map_for_output(output);
            let non_exclusive = layer_map.non_exclusive_zone();
            return Rect::new(
                non_exclusive.loc.x,
                non_exclusive.loc.y,
                non_exclusive.size.w as u32,
                non_exclusive.size.h as u32,
            );
        }
        Rect::new(0, 0, 1920, 1080)
    }
}
