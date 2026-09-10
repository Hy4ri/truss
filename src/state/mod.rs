pub mod rules;
pub mod window;
pub mod workspace;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub use rules::{WindowRule, WindowRuleAction, WindowRuleManager, WindowRuleMatcher};
pub use window::{Rect, Window, WindowId};
pub use workspace::Workspace;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StateError {
    #[error("Workspace with id {0} not found")]
    WorkspaceNotFound(u32),
    #[error("Window with id {0:?} not found")]
    WindowNotFound(WindowId),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

pub const SPECIAL_WORKSPACE_ID: u32 = 999;

/// Single source of truth for the compositor state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    pub workspaces: BTreeMap<u32, Workspace>,
    pub windows: BTreeMap<WindowId, Window>,
    pub active_workspace_id: u32,
    pub previous_workspace_id: Option<u32>,
    pub special_workspace_active: bool,
    pub focus_history: Vec<WindowId>,
    next_window_id: u64,
}

impl Default for State {
    fn default() -> Self {
        let mut workspaces = BTreeMap::new();
        for i in 1..=9 {
            workspaces.insert(i, Workspace::new(i, format!("{i}"), "master"));
        }
        workspaces.insert(
            SPECIAL_WORKSPACE_ID,
            Workspace::new(SPECIAL_WORKSPACE_ID, "special", "master"),
        );

        Self {
            workspaces,
            windows: BTreeMap::new(),
            active_workspace_id: 1,
            previous_workspace_id: None,
            special_workspace_active: false,
            focus_history: Vec::new(),
            next_window_id: 1,
        }
    }
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active_workspace(&self) -> &Workspace {
        self.workspaces
            .get(&self.active_workspace_id)
            .expect("active workspace must exist")
    }

    /// Returns the workspace that should be displayed on the given output name.
    pub fn active_workspace_for_output(&self, output_name: Option<&str>) -> &Workspace {
        let active = self.active_workspace();
        if active.output.as_deref() == output_name
            || (active.output.is_none() && output_name.is_none())
        {
            return active;
        }

        if let Some(name) = output_name {
            if let Some(ws) = self
                .workspaces
                .values()
                .find(|w| w.output.as_deref() == Some(name) && !w.windows.is_empty())
            {
                return ws;
            }
            if let Some(ws) = self
                .workspaces
                .values()
                .find(|w| w.output.as_deref() == Some(name))
            {
                return ws;
            }
        }

        active
    }

    pub fn active_workspace_mut(&mut self) -> &mut Workspace {
        self.workspaces
            .get_mut(&self.active_workspace_id)
            .expect("active workspace must exist")
    }

    pub fn switch_workspace(&mut self, id: u32) -> Result<(), StateError> {
        if !self.workspaces.contains_key(&id) {
            return Err(StateError::WorkspaceNotFound(id));
        }
        if self.active_workspace_id != id {
            self.previous_workspace_id = Some(self.active_workspace_id);
            self.active_workspace_id = id;
        }
        Ok(())
    }

    /// Calculate next workspace ID in cycling order (ignoring special workspaces).
    pub fn next_workspace_id(&self) -> Option<u32> {
        let keys: Vec<u32> = self
            .workspaces
            .keys()
            .copied()
            .filter(|&id| id != SPECIAL_WORKSPACE_ID)
            .collect();
        if keys.is_empty() {
            return None;
        }
        let idx = keys
            .iter()
            .position(|&id| id == self.active_workspace_id)
            .unwrap_or(0);
        let next_idx = (idx + 1) % keys.len();
        Some(keys[next_idx])
    }

    /// Calculate previous workspace ID in cycling order (ignoring special workspaces).
    pub fn prev_workspace_id(&self) -> Option<u32> {
        let keys: Vec<u32> = self
            .workspaces
            .keys()
            .copied()
            .filter(|&id| id != SPECIAL_WORKSPACE_ID)
            .collect();
        if keys.is_empty() {
            return None;
        }
        let idx = keys
            .iter()
            .position(|&id| id == self.active_workspace_id)
            .unwrap_or(0);
        let prev_idx = (idx + keys.len() - 1) % keys.len();
        Some(keys[prev_idx])
    }

    /// Toggle visibility of the overlay special / scratchpad workspace.
    pub fn toggle_special_workspace(&mut self) -> bool {
        self.special_workspace_active = !self.special_workspace_active;
        self.special_workspace_active
    }

    /// Record a window focus event in MRU order.
    pub fn record_focus(&mut self, id: WindowId) {
        self.focus_history.retain(|&w| w != id);
        self.focus_history.push(id);
    }

    /// Find the last active window on the requested workspace (or active workspace),
    /// excluding the currently focused window.
    pub fn last_focused_window(&self, workspace_id: Option<u32>) -> Option<WindowId> {
        let ws_id = workspace_id.unwrap_or(self.active_workspace_id);
        let current_focus = self.workspaces.get(&ws_id).and_then(|ws| ws.focused_window);

        // Check MRU focus history in reverse order
        for &win_id in self.focus_history.iter().rev() {
            if Some(win_id) != current_focus {
                if let Some(win) = self.windows.get(&win_id) {
                    if win.workspace_id == ws_id {
                        return Some(win_id);
                    }
                }
            }
        }

        // Fallback: check other windows in the workspace
        if let Some(ws) = self.workspaces.get(&ws_id) {
            for &win_id in ws.windows.iter().rev() {
                if Some(win_id) != current_focus && self.windows.contains_key(&win_id) {
                    return Some(win_id);
                }
            }
        }

        None
    }

    pub fn create_window(&mut self, workspace_id: Option<u32>) -> Result<WindowId, StateError> {
        let target_ws = workspace_id.unwrap_or(self.active_workspace_id);
        if !self.workspaces.contains_key(&target_ws) {
            return Err(StateError::WorkspaceNotFound(target_ws));
        }

        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;

        let window = Window::new(id, target_ws);
        self.windows.insert(id, window);

        let ws = self.workspaces.get_mut(&target_ws).unwrap();
        ws.add_window(id);

        if ws.focused_window == Some(id) {
            self.record_focus(id);
        }

        Ok(id)
    }

    pub fn remove_window(&mut self, id: WindowId) -> Result<(), StateError> {
        let window = self
            .windows
            .remove(&id)
            .ok_or(StateError::WindowNotFound(id))?;

        if let Some(ws) = self.workspaces.get_mut(&window.workspace_id) {
            ws.remove_window(id);
            if let Some(new_focus) = ws.focused_window {
                self.record_focus(new_focus);
            }
        }

        self.focus_history.retain(|&w| w != id);

        Ok(())
    }

    pub fn focus_window(&mut self, id: WindowId) -> Result<(), StateError> {
        let window = self
            .windows
            .get(&id)
            .ok_or(StateError::WindowNotFound(id))?;
        let ws_id = window.workspace_id;

        if ws_id != self.active_workspace_id {
            self.previous_workspace_id = Some(self.active_workspace_id);
            self.active_workspace_id = ws_id;
        }

        let ws = self.workspaces.get_mut(&ws_id).unwrap();
        ws.focused_window = Some(id);

        self.record_focus(id);

        Ok(())
    }

    pub fn move_window_to_workspace(
        &mut self,
        window_id: WindowId,
        target_ws_id: u32,
    ) -> Result<(), StateError> {
        if !self.workspaces.contains_key(&target_ws_id) {
            return Err(StateError::WorkspaceNotFound(target_ws_id));
        }

        let window = self
            .windows
            .get_mut(&window_id)
            .ok_or(StateError::WindowNotFound(window_id))?;
        let current_ws_id = window.workspace_id;

        if current_ws_id == target_ws_id {
            return Ok(());
        }

        if let Some(current_ws) = self.workspaces.get_mut(&current_ws_id) {
            current_ws.remove_window(window_id);
        }

        window.workspace_id = target_ws_id;
        let target_ws = self.workspaces.get_mut(&target_ws_id).unwrap();
        target_ws.add_window(window_id);

        Ok(())
    }

    pub fn toggle_floating(&mut self, id: WindowId) -> Result<bool, StateError> {
        let window = self
            .windows
            .get_mut(&id)
            .ok_or(StateError::WindowNotFound(id))?;
        window.floating = !window.floating;
        Ok(window.floating)
    }

    pub fn toggle_pinned(&mut self, id: WindowId) -> Result<bool, StateError> {
        let window = self
            .windows
            .get_mut(&id)
            .ok_or(StateError::WindowNotFound(id))?;
        window.pinned = !window.pinned;
        Ok(window.pinned)
    }

    pub fn toggle_fullscreen(&mut self, id: WindowId) -> Result<bool, StateError> {
        let window = self
            .windows
            .get_mut(&id)
            .ok_or(StateError::WindowNotFound(id))?;
        if !window.fullscreen {
            if window.saved_geometry.is_none() {
                window.saved_geometry = Some(window.geometry);
            }
            window.fullscreen = true;
        } else {
            window.fullscreen = false;
            if window.floating {
                if let Some(saved) = window.saved_geometry.take() {
                    window.geometry = saved;
                }
            } else {
                window.saved_geometry = None;
            }
        }
        Ok(window.fullscreen)
    }

    pub fn toggle_maximized(&mut self, id: WindowId) -> Result<bool, StateError> {
        let window = self
            .windows
            .get_mut(&id)
            .ok_or(StateError::WindowNotFound(id))?;
        if !window.maximized {
            if window.saved_geometry.is_none() {
                window.saved_geometry = Some(window.geometry);
            }
            window.maximized = true;
        } else {
            window.maximized = false;
            if window.floating {
                if let Some(saved) = window.saved_geometry.take() {
                    window.geometry = saved;
                }
            }
        }
        Ok(window.maximized)
    }

    pub fn set_window_geometry(&mut self, id: WindowId, rect: Rect) -> Result<(), StateError> {
        let window = self
            .windows
            .get_mut(&id)
            .ok_or(StateError::WindowNotFound(id))?;
        window.geometry = rect;
        Ok(())
    }
}
