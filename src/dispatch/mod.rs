pub mod command;
pub mod event;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use command::{Command, Direction};
pub use event::Event;

use crate::layout::{LayoutConfig, LayoutRegistry};
use crate::state::{Rect, State, StateError, WindowId};

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
    /// Window IDs whose close was requested while executing commands.
    ///
    /// The dispatcher is pure state (no access to Wayland surfaces), so
    /// `window.close` can only remove state here. Each execution door (IPC,
    /// keybinds) drains this queue afterwards and resolves the actual
    /// `ToplevelSurface` to `send_close()` the client. Queued (not executed
    /// inline) so command lists / macros stay atomic.
    pending_closes: Vec<WindowId>,
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
            pending_closes: Vec::new(),
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

    /// Take all window IDs queued for close by `Command::WindowClose`.
    ///
    /// Callers with surface access (IPC source, keybind handlers) must call
    /// this after every dispatch batch and `send_close()` each remaining
    /// `ToplevelSurface` — otherwise the client process leaks forever.
    pub fn take_pending_closes(&mut self) -> Vec<WindowId> {
        std::mem::take(&mut self.pending_closes)
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

        // For fullscreen windows, assign the full usable display area
        if let Some(ws) = state.workspaces.get(&workspace_id) {
            for &win_id in &ws.windows {
                if let Some(w) = state.windows.get_mut(&win_id) {
                    if w.fullscreen {
                        w.geometry = usable_area;
                    }
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

            Command::WindowClose { id } => {
                let win_id = match id {
                    Some(id) => id,
                    None => match state.active_workspace().focused_window {
                        Some(f) => f,
                        None => return Ok(DispatchResult::Ok),
                    },
                };
                state.remove_window(win_id)?;
                self.broadcast(&Event::WindowDestroyed { id: win_id });
                self.pending_closes.push(win_id);
                Ok(DispatchResult::Ok)
            }

            Command::WindowToggleFloating { id } => {
                let win_id = match id {
                    Some(id) => id,
                    None => match state.active_workspace().focused_window {
                        Some(f) => f,
                        None => return Ok(DispatchResult::Ok),
                    },
                };
                state.toggle_floating(win_id)?;
                let window = state
                    .windows
                    .get(&win_id)
                    .expect("window was just validated");
                self.broadcast(&Event::WindowStateChanged {
                    id: win_id,
                    floating: window.floating,
                    fullscreen: window.fullscreen,
                });
                Ok(DispatchResult::Ok)
            }

            Command::WindowToggleFullscreen { id } => {
                let win_id = match id {
                    Some(id) => id,
                    None => match state.active_workspace().focused_window {
                        Some(f) => f,
                        None => return Ok(DispatchResult::Ok),
                    },
                };
                state.toggle_fullscreen(win_id)?;
                let window = state
                    .windows
                    .get(&win_id)
                    .expect("window was just validated");
                self.broadcast(&Event::WindowStateChanged {
                    id: win_id,
                    floating: window.floating,
                    fullscreen: window.fullscreen,
                });
                Ok(DispatchResult::Ok)
            }

            Command::Spawn { command } => {
                let wayland_display =
                    std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "truss-0".into());
                crate::process::spawn_wayland_command(&command, &wayland_display).map_err(|e| {
                    DispatchError::InvalidParams(format!("Failed to spawn {command}: {e}"))
                })?;
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
                if self.layout_registry.get(&layout).is_none() {
                    return Err(DispatchError::InvalidParams(format!(
                        "Unknown layout '{layout}'"
                    )));
                }
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
                // Keep layout arithmetic safely inside signed compositor
                // coordinates even for malformed IPC input.
                let gap = gap.min(4_096);
                self.layout_config.gap = gap;
                self.broadcast(&Event::LayoutConfigChanged {
                    gap: Some(gap),
                    master_ratio: None,
                });
                Ok(DispatchResult::Ok)
            }

            Command::LayoutSetRatio { ratio } => {
                if !ratio.is_finite() {
                    return Err(DispatchError::InvalidParams(
                        "Master ratio must be a finite number".into(),
                    ));
                }
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
