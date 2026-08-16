use std::collections::HashMap;

use crate::state::{Rect, WindowId};

/// Parameters guiding dynamic layout geometry calculation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// Gap in pixels between windows and screen edges.
    pub gap: u32,
    /// Proportion of width assigned to the master window (0.0 to 1.0, e.g. 0.55).
    pub master_ratio: f32,
    /// Number of master windows (typically 1).
    pub master_count: u32,
}

use serde::{Deserialize, Serialize};

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            gap: 8,
            master_ratio: 0.55,
            master_count: 1,
        }
    }
}

/// A layout algorithm that calculates window geometries inside a usable screen area.
pub trait Layout: Send + Sync {
    /// Name of the layout (e.g. "master", "stack", "monocle", "grid").
    fn name(&self) -> &'static str;

    /// Calculate geometries for the given windows in order.
    ///
    /// - `windows`: Slice of window IDs to arrange in order (first window is Master).
    /// - `usable_area`: Bounding box of the display/workspace excluding panels/bars.
    /// - `config`: Gap, master ratio, and master count settings.
    ///
    /// Returns a list of `(WindowId, Rect)` pairs for each arranged window.
    fn arrange(
        &self,
        windows: &[WindowId],
        usable_area: Rect,
        config: &LayoutConfig,
    ) -> Vec<(WindowId, Rect)>;
}

/// Registry mapping layout names to layout implementations.
#[derive(Default)]
pub struct LayoutRegistry {
    layouts: HashMap<String, Box<dyn Layout>>,
}

impl LayoutRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            layouts: HashMap::new(),
        };
        registry.register(Box::new(MasterLayout));
        registry.register(Box::new(MonocleLayout));
        registry
    }

    pub fn register(&mut self, layout: Box<dyn Layout>) {
        self.layouts.insert(layout.name().to_string(), layout);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Layout> {
        self.layouts.get(name).map(|b| b.as_ref())
    }

    pub fn names(&self) -> Vec<&str> {
        self.layouts.keys().map(|s| s.as_str()).collect()
    }
}

pub mod master;
pub mod monocle;

pub use master::MasterLayout;
pub use monocle::MonocleLayout;
