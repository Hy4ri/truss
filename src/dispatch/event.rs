use serde::{Deserialize, Serialize};

use crate::state::WindowId;

/// Observable events broadcasted to all subscribers (Lua, IPC clients, internal).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event", content = "data")]
pub enum Event {
    #[serde(rename = "workspace.switched")]
    WorkspaceSwitched { id: u32 },

    #[serde(rename = "window.created")]
    WindowCreated { id: WindowId, workspace_id: u32 },

    #[serde(rename = "window.destroyed")]
    WindowDestroyed { id: WindowId },

    #[serde(rename = "window.focused")]
    WindowFocused { id: WindowId },

    #[serde(rename = "window.moved_workspace")]
    WindowMovedWorkspace { id: WindowId, workspace_id: u32 },

    #[serde(rename = "window.state_changed")]
    WindowStateChanged {
        id: WindowId,
        floating: bool,
        fullscreen: bool,
    },

    #[serde(rename = "layout.changed")]
    LayoutChanged { workspace_id: u32, layout: String },

    #[serde(rename = "layout.config_changed")]
    LayoutConfigChanged {
        gap: Option<u32>,
        master_ratio: Option<f32>,
    },

    #[serde(rename = "compositor.quitting")]
    CompositorQuitting,
}
