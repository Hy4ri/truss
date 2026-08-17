use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

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
    /// Name of the layout (e.g. "master", "stack", "monocle", "grid", "plugin:...").
    fn name(&self) -> &str;

    /// Calculate geometries for the given windows in order.
    fn arrange(
        &self,
        windows: &[WindowId],
        usable_area: Rect,
        config: &LayoutConfig,
    ) -> Vec<(WindowId, Rect)>;
}

/// Closure / function pointer based layout for API and plugin extensibility.
pub struct CustomLayout<F>
where
    F: Fn(&[WindowId], Rect, &LayoutConfig) -> Vec<(WindowId, Rect)> + Send + Sync + 'static,
{
    name: String,
    arranger: F,
}

impl<F> CustomLayout<F>
where
    F: Fn(&[WindowId], Rect, &LayoutConfig) -> Vec<(WindowId, Rect)> + Send + Sync + 'static,
{
    pub fn new(name: impl Into<String>, arranger: F) -> Self {
        Self {
            name: name.into(),
            arranger,
        }
    }
}

impl<F> Layout for CustomLayout<F>
where
    F: Fn(&[WindowId], Rect, &LayoutConfig) -> Vec<(WindowId, Rect)> + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn arrange(
        &self,
        windows: &[WindowId],
        usable_area: Rect,
        config: &LayoutConfig,
    ) -> Vec<(WindowId, Rect)> {
        (self.arranger)(windows, usable_area, config)
    }
}

/// Dynamic callback layout that can be plugged in at runtime (e.g. via Lua plugins).
pub type LayoutFn =
    Arc<dyn Fn(&[WindowId], Rect, &LayoutConfig) -> Vec<(WindowId, Rect)> + Send + Sync>;

pub struct PluginLayout {
    pub name: String,
    pub arranger: LayoutFn,
}

impl Layout for PluginLayout {
    fn name(&self) -> &str {
        &self.name
    }

    fn arrange(
        &self,
        windows: &[WindowId],
        usable_area: Rect,
        config: &LayoutConfig,
    ) -> Vec<(WindowId, Rect)> {
        (self.arranger)(windows, usable_area, config)
    }
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
        registry.register(Box::new(GridLayout));
        registry
    }

    pub fn register(&mut self, layout: Box<dyn Layout>) {
        self.layouts.insert(layout.name().to_string(), layout);
    }

    pub fn register_fn<F>(&mut self, name: impl Into<String>, func: F)
    where
        F: Fn(&[WindowId], Rect, &LayoutConfig) -> Vec<(WindowId, Rect)> + Send + Sync + 'static,
    {
        let layout = CustomLayout::new(name, func);
        self.register(Box::new(layout));
    }

    pub fn register_plugin(&mut self, name: impl Into<String>, arranger: LayoutFn) {
        let name = name.into();
        let plugin = PluginLayout {
            name: name.clone(),
            arranger,
        };
        self.register(Box::new(plugin));
    }

    pub fn get(&self, name: &str) -> Option<&dyn Layout> {
        self.layouts.get(name).map(|b| b.as_ref())
    }

    pub fn names(&self) -> Vec<&str> {
        self.layouts.keys().map(|s| s.as_str()).collect()
    }
}

pub mod grid;
pub mod master;
pub mod monocle;

pub use grid::GridLayout;
pub use master::MasterLayout;
pub use monocle::MonocleLayout;
