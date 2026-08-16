use serde::{Deserialize, Serialize};

use crate::state::Window;

/// Criteria to match windows for automated rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowRuleMatcher {
    /// Match app_id substring or exact string (case-insensitive)
    pub app_id: Option<String>,
    /// Match title substring or exact string (case-insensitive)
    pub title: Option<String>,
}

impl WindowRuleMatcher {
    pub fn matches(&self, window: &Window) -> bool {
        if let Some(ref req_app_id) = self.app_id {
            match &window.app_id {
                Some(app_id) => {
                    if !app_id.to_lowercase().contains(&req_app_id.to_lowercase()) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        if let Some(ref req_title) = self.title {
            match &window.title {
                Some(title) => {
                    if !title.to_lowercase().contains(&req_title.to_lowercase()) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        true
    }
}

/// Actions applied when a window matches a rule.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowRuleAction {
    /// Force window to open floating
    pub open_floating: Option<bool>,
    /// Force window to open on a specific workspace
    pub open_on_workspace: Option<u32>,
    /// Force window to open fullscreen
    pub open_fullscreen: Option<bool>,
}

/// A complete window rule with a matcher and corresponding actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowRule {
    pub name: String,
    pub match_criteria: WindowRuleMatcher,
    pub action: WindowRuleAction,
}

impl WindowRule {
    pub fn new(
        name: impl Into<String>,
        match_criteria: WindowRuleMatcher,
        action: WindowRuleAction,
    ) -> Self {
        Self {
            name: name.into(),
            match_criteria,
            action,
        }
    }

    /// Apply this rule's action to the target window.
    pub fn apply(&self, window: &mut Window) -> bool {
        if self.match_criteria.matches(window) {
            if let Some(floating) = self.action.open_floating {
                window.floating = floating;
            }
            if let Some(ws) = self.action.open_on_workspace {
                window.workspace_id = ws;
            }
            if let Some(fs) = self.action.open_fullscreen {
                window.fullscreen = fs;
            }
            true
        } else {
            false
        }
    }
}

/// Manager storing and evaluating window rules in order of registration.
#[derive(Debug, Clone, Default)]
pub struct WindowRuleManager {
    rules: Vec<WindowRule>,
}

impl WindowRuleManager {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn add_rule(&mut self, rule: WindowRule) {
        self.rules.push(rule);
    }

    pub fn clear(&mut self) {
        self.rules.clear();
    }

    /// Evaluate all rules against a window on creation / metadata update.
    pub fn evaluate_and_apply(&self, window: &mut Window) {
        for rule in &self.rules {
            rule.apply(window);
        }
    }
}
