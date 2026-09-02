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

/// Single source of truth for the compositor state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct State {
    pub workspaces: BTreeMap<u32, Workspace>,
    pub windows: BTreeMap<WindowId, Window>,
    pub active_workspace_id: u32,
    next_window_id: u64,
}

impl Default for State {
    fn default() -> Self {
        let mut workspaces = BTreeMap::new();
        for i in 1..=9 {
            workspaces.insert(i, Workspace::new(i, format!("{i}"), "master"));
        }

        Self {
            workspaces,
            windows: BTreeMap::new(),
            active_workspace_id: 1,
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

    pub fn active_workspace_mut(&mut self) -> &mut Workspace {
        self.workspaces
            .get_mut(&self.active_workspace_id)
            .expect("active workspace must exist")
    }

    pub fn switch_workspace(&mut self, id: u32) -> Result<(), StateError> {
        if !self.workspaces.contains_key(&id) {
            return Err(StateError::WorkspaceNotFound(id));
        }
        self.active_workspace_id = id;
        Ok(())
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

        Ok(id)
    }

    pub fn remove_window(&mut self, id: WindowId) -> Result<(), StateError> {
        let window = self
            .windows
            .remove(&id)
            .ok_or(StateError::WindowNotFound(id))?;

        if let Some(ws) = self.workspaces.get_mut(&window.workspace_id) {
            ws.remove_window(id);
        }

        Ok(())
    }

    pub fn focus_window(&mut self, id: WindowId) -> Result<(), StateError> {
        let window = self
            .windows
            .get(&id)
            .ok_or(StateError::WindowNotFound(id))?;
        let ws_id = window.workspace_id;

        if ws_id != self.active_workspace_id {
            self.active_workspace_id = ws_id;
        }

        let ws = self.workspaces.get_mut(&ws_id).unwrap();
        ws.focused_window = Some(id);

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

    pub fn toggle_fullscreen(&mut self, id: WindowId) -> Result<bool, StateError> {
        let window = self
            .windows
            .get_mut(&id)
            .ok_or(StateError::WindowNotFound(id))?;
        window.fullscreen = !window.fullscreen;
        Ok(window.fullscreen)
    }

    pub fn toggle_maximized(&mut self, id: WindowId) -> Result<bool, StateError> {
        let window = self
            .windows
            .get_mut(&id)
            .ok_or(StateError::WindowNotFound(id))?;
        window.maximized = !window.maximized;
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
