use serde::{Deserialize, Serialize};

/// Per-device configuration options (mouse, touchpad, keyboard, etc.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub name: String,
    pub sensitivity: Option<f64>,
    pub accel_profile: Option<String>,
    pub natural_scroll: Option<bool>,
    pub tap_to_click: Option<bool>,
    pub disable_while_typing: Option<bool>,
    pub enabled: Option<bool>,
}

/// Gesture configuration (e.g. 3-finger horizontal swipe -> workspace change)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GestureConfig {
    pub fingers: u32,
    pub direction: String,
    pub action: String,
}
