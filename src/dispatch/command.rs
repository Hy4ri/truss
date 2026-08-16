use serde::{Deserialize, Serialize};

use crate::state::WindowId;

/// Strongly typed commands that can be issued to the compositor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "command", content = "params")]
pub enum Command {
    #[serde(rename = "workspace.switch")]
    WorkspaceSwitch { id: u32 },

    #[serde(rename = "window.focus")]
    WindowFocus { id: WindowId },

    #[serde(rename = "window.focus_dir")]
    WindowFocusDir { direction: Direction },

    #[serde(rename = "window.swap_master")]
    WindowSwapMaster,

    #[serde(rename = "window.move_to_workspace")]
    WindowMoveToWorkspace {
        window_id: Option<WindowId>,
        workspace_id: u32,
    },

    #[serde(rename = "layout.set")]
    LayoutSet { layout: String },

    #[serde(rename = "state.get")]
    StateGet,

    #[serde(rename = "compositor.quit")]
    CompositorQuit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Next,
    Prev,
}
