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
    /// Execute an external shell process (e.g. "kitty", "alacritty", "rofi")
    Spawn(String),
}

/// Registry mapping keyboard shortcuts to actions.
pub struct Keybindings {
    bindings: HashMap<KeyPattern, KeyAction>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self::new()
    }
}

impl Keybindings {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
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
                crate::process::spawn_wayland_command(cmd, &wayland_display).map_err(|e| {
                    crate::dispatch::DispatchError::InvalidParams(format!(
                        "Failed to spawn {cmd}: {e}"
                    ))
                })?;
                Ok(crate::dispatch::DispatchResult::Ok)
            }
        }
    }
}

/// Map a key name to an XKB keysym (subset used in keybindings).
///
/// Supports single ASCII letters/digits, named keys (Return, Escape, Tab,
/// Space, BackSpace, Delete, Insert, Home, End, Page_Up/Prior, Page_Down/Next,
/// arrows), punctuation keys (comma, period, slash, semicolon, apostrophe,
/// minus, equal, grave, backslash, bracketleft, bracketright, plus) and
/// F1..=F24.
pub fn keysym_from_name(name: &str) -> Option<u32> {
    let name = name.trim().to_lowercase();

    // Single ASCII character: letters a-z / A-Z and digits 0-9.
    if name.len() == 1 {
        let c = name.as_bytes()[0];
        return match c {
            b'a'..=b'z' => Some(0x61 + (c - b'a') as u32),
            b'0'..=b'9' => Some(0x30 + (c - b'0') as u32),
            _ => None,
        };
    }

    let named = match name.as_str() {
        "return" | "enter" => 0xff0d,
        "escape" => 0xff1b,
        "tab" => 0xff09,
        "space" => 0x0020,
        "backspace" => 0xff08,
        "delete" => 0xffff,
        "insert" => 0xff63,
        "home" => 0xff50,
        "end" => 0xff57,
        "page_up" | "prior" => 0xff55,
        "page_down" | "next" => 0xff56,
        "left" => 0xff51,
        "up" => 0xff52,
        "right" => 0xff53,
        "down" => 0xff54,
        "comma" => 0x002c,
        "period" => 0x002e,
        "slash" => 0x002f,
        "semicolon" => 0x003b,
        "apostrophe" => 0x0027,
        "minus" => 0x002d,
        "equal" => 0x003d,
        "grave" => 0x0060,
        "backslash" => 0x005c,
        "bracketleft" => 0x005b,
        "bracketright" => 0x005d,
        "plus" => 0x002b,
        _ => {
            // F1..=F24 -> 0xffbe..=0xffd5
            let f = name.strip_prefix('f')?;
            let n: u32 = f.parse().ok()?;
            if (1..=24).contains(&n) {
                0xffbe + (n - 1)
            } else {
                return None;
            }
        }
    };
    Some(named)
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
