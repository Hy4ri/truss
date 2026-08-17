use std::collections::HashMap;
use tracing::info;

use crate::dispatch::Command;

/// Modifiers held down during a key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub logo: bool, // Super / Mod4
}

impl Modifiers {
    pub const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        logo: false,
    };

    pub const SUPER: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        logo: true,
    };

    pub const SUPER_SHIFT: Self = Self {
        ctrl: false,
        alt: false,
        shift: true,
        logo: true,
    };

    pub fn matches(&self, other: &Self) -> bool {
        self.ctrl == other.ctrl
            && self.alt == other.alt
            && self.shift == other.shift
            && self.logo == other.logo
    }
}

/// A combination of key modifiers + keysym (or raw keysym value) mapped to a Dispatcher Command.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyPattern {
    pub modifiers: Modifiers,
    /// X11/XKB Keysym value (e.g. 0x0071 for 'q', 0xff0d for Enter, etc.)
    pub keysym: u32,
}

impl KeyPattern {
    pub fn new(modifiers: Modifiers, keysym: u32) -> Self {
        Self { modifiers, keysym }
    }
}

/// Dynamic Keybinding action definition.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyAction {
    /// Dispatch an internal strongly typed Command
    Dispatch(Command),
    /// Execute an external shell process (e.g. "foot", "alacritty", "rofi")
    Spawn(String),
}

/// Registry mapping keyboard shortcuts to actions.
pub struct Keybindings {
    bindings: HashMap<KeyPattern, KeyAction>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self::new_default()
    }
}

impl Keybindings {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Default ergonomic tiling keybindings (Super+Enter, Super+q, Super+1..9, etc.)
    pub fn new_default() -> Self {
        let mut kb = Self::new();

        // XKB Keysym constants
        const KEY_RETURN: u32 = 0xff0d;
        const KEY_Q: u32 = 0x0071;
        const KEY_D: u32 = 0x0064;
        const KEY_F: u32 = 0x0066;
        const KEY_J: u32 = 0x006a;
        const KEY_K: u32 = 0x006b;
        const KEY_SPACE: u32 = 0x0020;
        const KEY_1: u32 = 0x0031;

        // Super + Return -> Spawn terminal
        kb.bind(
            KeyPattern::new(Modifiers::SUPER, KEY_RETURN),
            KeyAction::Spawn("foot".into()),
        );

        // Super + D -> App Launcher
        kb.bind(
            KeyPattern::new(Modifiers::SUPER, KEY_D),
            KeyAction::Spawn("fuzzel || rofi -show drun || wofi".into()),
        );

        // Super + Q -> Close focused window
        kb.bind(
            KeyPattern::new(Modifiers::SUPER, KEY_Q),
            KeyAction::Dispatch(Command::WindowClose { id: None }),
        );

        // Super + Shift + Q -> Quit compositor
        kb.bind(
            KeyPattern::new(Modifiers::SUPER_SHIFT, KEY_Q),
            KeyAction::Dispatch(Command::CompositorQuit),
        );

        // Super + F -> Toggle fullscreen
        kb.bind(
            KeyPattern::new(Modifiers::SUPER, KEY_F),
            KeyAction::Dispatch(Command::WindowToggleFullscreen { id: None }),
        );

        // Super + Shift + Space -> Toggle floating
        kb.bind(
            KeyPattern::new(Modifiers::SUPER_SHIFT, KEY_SPACE),
            KeyAction::Dispatch(Command::WindowToggleFloating { id: None }),
        );

        // Super + j / k -> Focus Next / Prev
        kb.bind(
            KeyPattern::new(Modifiers::SUPER, KEY_J),
            KeyAction::Dispatch(Command::WindowFocusDir {
                direction: crate::dispatch::Direction::Next,
            }),
        );
        kb.bind(
            KeyPattern::new(Modifiers::SUPER, KEY_K),
            KeyAction::Dispatch(Command::WindowFocusDir {
                direction: crate::dispatch::Direction::Prev,
            }),
        );

        // Super + Space -> Swap with Master
        kb.bind(
            KeyPattern::new(Modifiers::SUPER, KEY_SPACE),
            KeyAction::Dispatch(Command::WindowSwapMaster),
        );

        // Super + 1..9 -> Switch Workspace
        // Super + Shift + 1..9 -> Move focused window to Workspace
        for ws in 1..=9 {
            let key = KEY_1 + (ws - 1);
            kb.bind(
                KeyPattern::new(Modifiers::SUPER, key),
                KeyAction::Dispatch(Command::WorkspaceSwitch { id: ws }),
            );
            kb.bind(
                KeyPattern::new(Modifiers::SUPER_SHIFT, key),
                KeyAction::Dispatch(Command::WindowMoveToWorkspace {
                    window_id: None,
                    workspace_id: ws,
                }),
            );
        }

        kb
    }

    pub fn bind(&mut self, pattern: KeyPattern, action: KeyAction) {
        self.bindings.insert(pattern, action);
    }

    pub fn unbind(&mut self, pattern: &KeyPattern) -> bool {
        self.bindings.remove(pattern).is_some()
    }

    pub fn match_action(&self, modifiers: Modifiers, keysym: u32) -> Option<&KeyAction> {
        let pattern = KeyPattern::new(modifiers, keysym);
        if let Some(action) = self.bindings.get(&pattern) {
            return Some(action);
        }

        // Case folding: 'A'..='Z' (0x41..=0x5a) -> 'a'..='z' (0x61..=0x7a)
        if (0x0041..=0x005a).contains(&keysym) {
            let lower_pattern = KeyPattern::new(modifiers, keysym + 0x20);
            if let Some(action) = self.bindings.get(&lower_pattern) {
                return Some(action);
            }
        }

        // Case folding: 'a'..='z' (0x61..=0x7a) -> 'A'..='Z' (0x41..=0x5a)
        if (0x0061..=0x007a).contains(&keysym) {
            let upper_pattern = KeyPattern::new(modifiers, keysym - 0x20);
            if let Some(action) = self.bindings.get(&upper_pattern) {
                return Some(action);
            }
        }

        None
    }

    pub fn execute_action(
        &self,
        action: &KeyAction,
        dispatcher: &mut crate::dispatch::Dispatcher,
        state: &mut crate::state::State,
    ) -> Result<crate::dispatch::DispatchResult, crate::dispatch::DispatchError> {
        match action {
            KeyAction::Dispatch(cmd) => {
                info!("Keybinding triggered dispatch: {:?}", cmd);
                dispatcher.dispatch(state, cmd.clone())
            }
            KeyAction::Spawn(cmd) => {
                info!("Keybinding triggered spawn: {}", cmd);
                let wayland_display =
                    std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "truss-0".into());
                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(cmd)
                    .env("WAYLAND_DISPLAY", &wayland_display)
                    .spawn()
                    .map_err(|e| {
                        crate::dispatch::DispatchError::InvalidParams(format!(
                            "Failed to spawn {cmd}: {e}"
                        ))
                    })?;
                Ok(crate::dispatch::DispatchResult::Ok)
            }
        }
    }
}

/// Helper function to parse virtual terminal switch requests from keyboard events.
/// Handles:
/// 1. XKB XF86Switch_VT_1..12 keysyms (0x1008FE01..=0x1008FE0C)
/// 2. Ctrl + Alt + F1..F12 keysyms (0xffbe..=0xffc9)
/// 3. Ctrl + Alt + raw Linux evdev keycodes (59..=68 for F1-F10, 87 for F11, 88 for F12)
pub fn parse_vt_switch(modifiers: Modifiers, sym: u32, raw_sym: u32, key_code: u32) -> Option<i32> {
    // 1. XKB XF86Switch_VT_1..12 keysyms
    if (0x1008_fe01..=0x1008_fe0c).contains(&sym) {
        return Some((sym - 0x1008_fe01 + 1) as i32);
    }
    if (0x1008_fe01..=0x1008_fe0c).contains(&raw_sym) {
        return Some((raw_sym - 0x1008_fe01 + 1) as i32);
    }

    // 2. Ctrl + Alt + F1..F12 keysyms
    if modifiers.ctrl && modifiers.alt {
        if (0xffbe..=0xffc9).contains(&sym) {
            return Some((sym - 0xffbe + 1) as i32);
        }
        if (0xffbe..=0xffc9).contains(&raw_sym) {
            return Some((raw_sym - 0xffbe + 1) as i32);
        }

        // 3. Ctrl + Alt + raw evdev keycodes
        match key_code {
            59..=68 => return Some((key_code - 59 + 1) as i32),
            87 => return Some(11),
            88 => return Some(12),
            _ => {}
        }
    }

    None
}
