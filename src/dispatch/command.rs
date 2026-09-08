use serde::{Deserialize, Serialize};

use crate::state::WindowId;

/// Strongly typed commands that can be issued to the compositor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "command", content = "params")]
pub enum Command {
    #[serde(rename = "workspace.switch")]
    WorkspaceSwitch { id: u32 },

    #[serde(rename = "workspace.next")]
    WorkspaceNext,

    #[serde(rename = "workspace.prev")]
    WorkspacePrev,

    #[serde(rename = "workspace.previous")]
    WorkspacePrevious,

    #[serde(rename = "workspace.toggle_special")]
    WorkspaceToggleSpecial,

    #[serde(rename = "window.move_to_special")]
    WindowMoveToSpecial { window_id: Option<WindowId> },

    #[serde(rename = "render.zoom_change")]
    ZoomChange { delta: f32 },

    #[serde(rename = "render.zoom_reset")]
    ZoomReset,

    #[serde(rename = "output.dpms_toggle")]
    DpmsToggle,

    #[serde(rename = "workspace.move_to_monitor")]
    WorkspaceMoveToMonitor {
        workspace_id: Option<u32>,
        monitor: String,
    },

    #[serde(rename = "group.toggle")]
    GroupToggle,

    #[serde(rename = "group.next")]
    GroupNext,

    #[serde(rename = "group.prev")]
    GroupPrev,

    #[serde(rename = "window.focus")]
    WindowFocus { id: WindowId },

    #[serde(rename = "window.focus_dir")]
    WindowFocusDir { direction: Direction },

    #[serde(rename = "window.focus_last")]
    WindowFocusLast,

    #[serde(rename = "window.swap_master")]
    WindowSwapMaster,

    #[serde(rename = "window.close")]
    WindowClose { id: Option<WindowId> },

    #[serde(rename = "window.force_kill")]
    WindowForceKill { id: Option<WindowId> },

    #[serde(rename = "window.toggle_floating")]
    WindowToggleFloating { id: Option<WindowId> },

    #[serde(rename = "window.toggle_pin")]
    WindowTogglePin { id: Option<WindowId> },

    #[serde(rename = "window.toggle_fullscreen")]
    WindowToggleFullscreen { id: Option<WindowId> },

    #[serde(rename = "window.toggle_maximize")]
    WindowToggleMaximize { id: Option<WindowId> },

    #[serde(rename = "window.move_to_workspace")]
    WindowMoveToWorkspace {
        window_id: Option<WindowId>,
        workspace_id: u32,
    },

    #[serde(rename = "window.move_to_workspace_silent")]
    WindowMoveToWorkspaceSilent {
        window_id: Option<WindowId>,
        workspace_id: u32,
    },

    #[serde(rename = "layout.set")]
    LayoutSet { layout: String },

    #[serde(rename = "layout.set_gap")]
    LayoutSetGap { gap: u32 },

    #[serde(rename = "layout.set_ratio")]
    LayoutSetRatio { ratio: f32 },

    #[serde(rename = "spawn")]
    Spawn { command: String },

    #[serde(rename = "config.reload")]
    ConfigReload,

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
