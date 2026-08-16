pub mod command;
pub mod event;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use command::{Command, Direction};
pub use event::Event;

use crate::layout::{LayoutConfig, LayoutRegistry};
use crate::state::{Rect, State, StateError};

#[derive(Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchError {
    #[error("State error: {0}")]
    State(String),
    #[error("Command not recognized or unsupported: {0}")]
    UnknownCommand(String),
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
}

impl From<StateError> for DispatchError {
    fn from(err: StateError) -> Self {
        Self::State(err.to_string())
    }
}

/// Result returned from executing a command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DispatchResult {
    Ok,
    State(crate::state::State),
    Quitting,
}

pub type Subscriber = Box<dyn FnMut(&Event) + Send + 'static>;

/// Central Command Dispatcher. Validates, executes commands against State, applies Layout calculations, and broadcasts Events.
pub struct Dispatcher {
    subscribers: Vec<Subscriber>,
    pub layout_registry: LayoutRegistry,
    pub layout_config: LayoutConfig,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
            layout_registry: LayoutRegistry::new(),
            layout_config: LayoutConfig::default(),
        }
    }

    pub fn subscribe<F>(&mut self, callback: F)
    where
        F: FnMut(&Event) + Send + 'static,
    {
        self.subscribers.push(Box::new(callback));
    }

    pub fn broadcast(&mut self, event: &Event) {
        for sub in &mut self.subscribers {
            sub(event);
        }
    }

    /// Recalculate geometries of all tiled windows on a given workspace within a usable display area.
    pub fn recalculate_workspace_layout(
        &self,
        state: &mut State,
        workspace_id: u32,
        usable_area: Rect,
    ) {
        let (layout_name, window_ids) = match state.workspaces.get(&workspace_id) {
            Some(ws) => (ws.layout.clone(), ws.windows.clone()),
            None => return,
        };

        // Filter out floating/fullscreen windows if needed, or arrange all tiled
        let tiled_windows: Vec<_> = window_ids
            .into_iter()
            .filter(|id| {
                state
                    .windows
                    .get(id)
                    .map(|w| !w.floating && !w.fullscreen)
                    .unwrap_or(false)
            })
            .collect();

        if let Some(layout) = self.layout_registry.get(&layout_name) {
            let geometries = layout.arrange(&tiled_windows, usable_area, &self.layout_config);
            for (win_id, rect) in geometries {
                if let Some(w) = state.windows.get_mut(&win_id) {
                    w.geometry = rect;
                }
            }
        }
    }

    /// Primary execution path: takes a command and mutable state, applies it atomically,
    /// and broadcasts the resulting event if state changed.
    pub fn dispatch(
        &mut self,
        state: &mut State,
        command: Command,
    ) -> Result<DispatchResult, DispatchError> {
        match command {
            Command::WorkspaceSwitch { id } => {
                let prev_id = state.active_workspace_id;
                state.switch_workspace(id)?;
                if prev_id != id {
                    self.broadcast(&Event::WorkspaceSwitched { id });
                }
                Ok(DispatchResult::Ok)
            }

            Command::WindowFocus { id } => {
                state.focus_window(id)?;
                self.broadcast(&Event::WindowFocused { id });
                Ok(DispatchResult::Ok)
            }

            Command::WindowFocusDir { direction } => {
                let ws = state.active_workspace_mut();
                let focused = match direction {
                    Direction::Next => ws.focus_next(),
                    Direction::Prev => ws.focus_prev(),
                };

                if let Some(id) = focused {
                    self.broadcast(&Event::WindowFocused { id });
                }
                Ok(DispatchResult::Ok)
            }

            Command::WindowSwapMaster => {
                let ws = state.active_workspace_mut();
                if ws.swap_focused_with_master() {
                    if let Some(master_id) = ws.windows.first() {
                        self.broadcast(&Event::WindowFocused { id: *master_id });
                    }
                }
                Ok(DispatchResult::Ok)
            }

            Command::WindowMoveToWorkspace {
                window_id,
                workspace_id,
            } => {
                let win_id = match window_id {
                    Some(id) => id,
                    None => state.active_workspace().focused_window.ok_or_else(|| {
                        DispatchError::InvalidParams("No focused window to move".into())
                    })?,
                };

                state.move_window_to_workspace(win_id, workspace_id)?;
                self.broadcast(&Event::WindowMovedWorkspace {
                    id: win_id,
                    workspace_id,
                });
                Ok(DispatchResult::Ok)
            }

            Command::LayoutSet { layout } => {
                let ws_id = state.active_workspace_id;
                let ws = state.active_workspace_mut();
                ws.layout = layout.clone();
                self.broadcast(&Event::LayoutChanged {
                    workspace_id: ws_id,
                    layout,
                });
                Ok(DispatchResult::Ok)
            }

            Command::LayoutSetGap { gap } => {
                self.layout_config.gap = gap;
                self.broadcast(&Event::LayoutConfigChanged {
                    gap: Some(gap),
                    master_ratio: None,
                });
                Ok(DispatchResult::Ok)
            }

            Command::LayoutSetRatio { ratio } => {
                let clamped = ratio.clamp(0.1, 0.9);
                self.layout_config.master_ratio = clamped;
                self.broadcast(&Event::LayoutConfigChanged {
                    gap: None,
                    master_ratio: Some(clamped),
                });
                Ok(DispatchResult::Ok)
            }

            Command::StateGet => Ok(DispatchResult::State(state.clone())),

            Command::CompositorQuit => {
                self.broadcast(&Event::CompositorQuitting);
                Ok(DispatchResult::Quitting)
            }
        }
    }
}
