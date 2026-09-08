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

fn matches_pattern(candidate: &str, pattern: &str) -> bool {
    let cand_lower = candidate.to_lowercase();
    let pat = pattern
        .trim_start_matches('^')
        .trim_end_matches('$')
        .trim_start_matches('(')
        .trim_end_matches(')');
    if pat.contains('|') {
        pat.split('|').any(|part| {
            let p = part.trim();
            !p.is_empty() && cand_lower.contains(&p.to_lowercase())
        })
    } else {
        cand_lower.contains(&pat.to_lowercase())
    }
}

impl WindowRuleMatcher {
    pub fn matches(&self, window: &Window) -> bool {
        if let Some(ref req_app_id) = self.app_id {
            match &window.app_id {
                Some(app_id) => {
                    if !matches_pattern(app_id, req_app_id) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        if let Some(ref req_title) = self.title {
            match &window.title {
                Some(title) => {
                    if !matches_pattern(title, req_title) {
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
    /// Automatically center floating window on screen
    pub center: Option<bool>,
    /// Pin window across all workspaces (sticky)
    pub pin: Option<bool>,
    /// Initial size (pixels or percentage like "800 600" or "25% 25%")
    pub initial_size: Option<String>,
    /// Initial move/position (pixels or percentage like "100 100" or "72% 7%")
    pub initial_position: Option<String>,
    /// Custom opacity stored as percent 0..=100
    pub opacity: Option<u8>,
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
            if let Some(center) = self.action.center {
                window.center = center;
            }
            if let Some(pin) = self.action.pin {
                window.pinned = pin;
            }
            if self.action.initial_size.is_some() {
                window.initial_size = self.action.initial_size.clone();
            }
            if self.action.initial_position.is_some() {
                window.initial_position = self.action.initial_position.clone();
            }
            if let Some(op) = self.action.opacity {
                window.opacity = Some(op);
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
