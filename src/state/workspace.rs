use serde::{Deserialize, Serialize};

use super::window::WindowId;

/// Represents a virtual desktop workspace containing windows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub id: u32,
    pub name: String,
    pub layout: String,
    pub windows: Vec<WindowId>,
    pub focused_window: Option<WindowId>,
}

impl Workspace {
    pub fn new(id: u32, name: impl Into<String>, layout: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            layout: layout.into(),
            windows: Vec::new(),
            focused_window: None,
        }
    }

    pub fn add_window(&mut self, id: WindowId) {
        if !self.windows.contains(&id) {
            self.windows.push(id);
        }
        if self.focused_window.is_none() {
            self.focused_window = Some(id);
        }
    }

    pub fn remove_window(&mut self, id: WindowId) -> bool {
        let prev_len = self.windows.len();
        self.windows.retain(|&w| w != id);
        if self.focused_window == Some(id) {
            self.focused_window = self.windows.last().copied();
        }
        self.windows.len() < prev_len
    }

    pub fn focus_next(&mut self) -> Option<WindowId> {
        if self.windows.is_empty() {
            self.focused_window = None;
            return None;
        }

        let next_idx = match self.focused_window {
            Some(current) => {
                let idx = self.windows.iter().position(|&w| w == current).unwrap_or(0);
                (idx + 1) % self.windows.len()
            }
            None => 0,
        };

        let new_focus = self.windows[next_idx];
        self.focused_window = Some(new_focus);
        Some(new_focus)
    }

    pub fn focus_prev(&mut self) -> Option<WindowId> {
        if self.windows.is_empty() {
            self.focused_window = None;
            return None;
        }

        let prev_idx = match self.focused_window {
            Some(current) => {
                let idx = self.windows.iter().position(|&w| w == current).unwrap_or(0);
                if idx == 0 {
                    self.windows.len() - 1
                } else {
                    idx - 1
                }
            }
            None => 0,
        };

        let new_focus = self.windows[prev_idx];
        self.focused_window = Some(new_focus);
        Some(new_focus)
    }

    pub fn swap_focused_with_master(&mut self) -> bool {
        if self.windows.len() < 2 {
            return false;
        }

        if let Some(focused) = self.focused_window {
            if let Some(idx) = self.windows.iter().position(|&w| w == focused) {
                if idx != 0 {
                    self.windows.swap(0, idx);
                    return true;
                }
            }
        }
        false
    }
}
