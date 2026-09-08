use serde::{Deserialize, Serialize};

/// Unique identifier for a managed window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u64);

/// Logical 2D rectangle in compositor coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Metadata and layout properties of a managed window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Window {
    pub id: WindowId,
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub workspace_id: u32,
    pub geometry: Rect,
    pub floating: bool,
    pub center: bool,
    pub pinned: bool,
    pub fullscreen: bool,
    pub maximized: bool,
    pub saved_geometry: Option<Rect>,
    pub initial_size: Option<String>,
    pub initial_position: Option<String>,
    /// Opacity stored as integer percent (0 to 100) to keep Eq/Hash implementations happy
    pub opacity: Option<u8>,
    pub idle_inhibit: Option<String>,
}

impl Window {
    pub fn new(id: WindowId, workspace_id: u32) -> Self {
        Self {
            id,
            app_id: None,
            title: None,
            workspace_id,
            geometry: Rect::default(),
            floating: false,
            center: false,
            pinned: false,
            fullscreen: false,
            maximized: false,
            saved_geometry: None,
            initial_size: None,
            initial_position: None,
            opacity: None,
            idle_inhibit: None,
        }
    }
}
