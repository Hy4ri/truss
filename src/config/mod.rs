pub mod plugins;

use mlua::{Lua, LuaSerdeExt};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

pub use plugins::LuaPluginManager;

use crate::dispatch::{Command, Event};
use crate::input::{KeyAction, KeyPattern, Keybindings, Modifiers};
use crate::layout::LayoutRegistry;
use crate::state::{WindowId, WindowRule, WindowRuleAction, WindowRuleManager, WindowRuleMatcher};

/// The built-in default configuration, used when no config file exists.
pub const DEFAULT_CONFIG: &str = include_str!("../../resources/config.default.lua");

/// Where the configuration was resolved from.
#[derive(Debug)]
pub enum ConfigSource {
    File(PathBuf),
    Embedded,
}

/// Lua Configuration Runtime environment for truss.
pub struct LuaConfig {
    lua: Lua,
}

impl Default for LuaConfig {
    fn default() -> Self {
        Self::new().expect("Failed to initialize Lua state")
    }
}

impl LuaConfig {
    pub fn new() -> Result<Self, mlua::Error> {
        let lua = Lua::new();
        let config = Self { lua };
        config.register_truss_api()?;
        Ok(config)
    }

    /// Register the global `truss` table with helper methods and submodules.
    fn register_truss_api(&self) -> Result<(), mlua::Error> {
        let globals = self.lua.globals();
        let truss = self.lua.create_table()?;

        // Hooks table for event callbacks
        let hooks = self.lua.create_table()?;
        self.lua.set_named_registry_value("_truss_hooks", hooks)?;

        // Rules table for window rules
        let rules = self.lua.create_table()?;
        self.lua.set_named_registry_value("_truss_rules", rules)?;

        // Autostart commands table
        let autostart = self.lua.create_table()?;
        self.lua
            .set_named_registry_value("_truss_autostart", autostart)?;

        // Keybindings table
        let keybinds = self.lua.create_table()?;
        self.lua
            .set_named_registry_value("_truss_keybinds", keybinds)?;

        // Settings table
        let settings = self.lua.create_table()?;
        self.lua
            .set_named_registry_value("_truss_settings", settings)?;

        // truss.version
        truss.set("version", env!("CARGO_PKG_VERSION"))?;

        // Helper: truss.source(path) for modular includes
        let lua_clone = self.lua.clone();
        let source_fn = self.lua.create_function(move |_, path_str: String| {
            let path = Path::new(&path_str);
            let content = std::fs::read_to_string(path).map_err(|e| {
                mlua::Error::RuntimeError(format!("Failed to read {path_str}: {e}"))
            })?;
            lua_clone.load(&content).set_name(&path_str).exec()
        })?;
        truss.set("source", source_fn)?;

        // Helper: truss.spawn(command) - immediate spawn
        let spawn_fn = self.lua.create_function(|_, cmd: String| {
            let wayland_display =
                std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "truss-0".into());
            let _ = crate::process::spawn_wayland_command(&cmd, &wayland_display);
            Ok(())
        })?;
        truss.set("spawn", spawn_fn)?;

        // Helper: truss.spawn_at_startup(command) - deferred launch on compositor ready
        let lua_for_autostart = self.lua.clone();
        let autostart_fn = self.lua.create_function(move |_, cmd: String| {
            let autostart: mlua::Table =
                lua_for_autostart.named_registry_value("_truss_autostart")?;
            let len = autostart.raw_len();
            autostart.set(len + 1, cmd)?;
            Ok(())
        })?;
        truss.set("spawn_at_startup", autostart_fn)?;

        // Helper: truss.on(event_name, callback)
        let lua_for_on = self.lua.clone();
        let on_fn = self.lua.create_function(
            move |_, (event_name, callback): (String, mlua::Function)| {
                let hooks: mlua::Table = lua_for_on.named_registry_value("_truss_hooks")?;
                let list: mlua::Table = match hooks.get(event_name.as_str())? {
                    mlua::Value::Table(t) => t,
                    _ => {
                        let t = lua_for_on.create_table()?;
                        hooks.set(event_name.clone(), t.clone())?;
                        t
                    }
                };
                let len = list.raw_len();
                list.set(len + 1, callback)?;
                Ok(())
            },
        )?;
        truss.set("on", on_fn)?;

        // Helper: truss.window_rule(name, rule_table)
        let lua_for_rules = self.lua.clone();
        let rule_fn =
            self.lua
                .create_function(move |_, (name, rule_table): (String, mlua::Table)| {
                    let rules: mlua::Table = lua_for_rules.named_registry_value("_truss_rules")?;
                    let len = rules.raw_len();
                    let entry = lua_for_rules.create_table()?;
                    entry.set("name", name)?;
                    entry.set("rule", rule_table)?;
                    rules.set(len + 1, entry)?;
                    Ok(())
                })?;
        truss.set("window_rule", rule_fn)?;

        // Helper: truss.keybind(mods, key, action) - register a keybinding
        let lua_for_keybinds = self.lua.clone();
        let keybind_fn = self.lua.create_function(
            move |_, (mods, key, action): (String, String, mlua::Value)| {
                let keybinds: mlua::Table =
                    lua_for_keybinds.named_registry_value("_truss_keybinds")?;
                let entry = lua_for_keybinds.create_table()?;
                entry.set("mods", mods)?;
                entry.set("key", key)?;
                entry.set("action", action)?;
                let len = keybinds.raw_len();
                keybinds.set(len + 1, entry)?;
                Ok(())
            },
        )?;
        truss.set("keybind", keybind_fn)?;

        // Helper: truss.set(name, value) - register a compositor setting
        let lua_for_settings = self.lua.clone();
        let set_fn = self
            .lua
            .create_function(move |_, (name, value): (String, mlua::Value)| {
                let settings: mlua::Table =
                    lua_for_settings.named_registry_value("_truss_settings")?;
                settings.set(name, value)?;
                Ok(())
            })?;
        truss.set("set", set_fn)?;

        // Command constructors
        let cmd_table = self.lua.create_table()?;

        let ws_switch = self.lua.create_function(|lua, id: u32| {
            let cmd = Command::WorkspaceSwitch { id };
            lua.to_value(&cmd)
        })?;
        cmd_table.set("workspace_switch", ws_switch)?;

        let move_to_ws = self.lua.create_function(|lua, ws: u32| {
            let cmd = Command::WindowMoveToWorkspace {
                window_id: None,
                workspace_id: ws,
            };
            lua.to_value(&cmd)
        })?;
        cmd_table.set("move_to_workspace", move_to_ws)?;

        let win_focus_dir = self.lua.create_function(|lua, dir: String| {
            let direction = match dir.to_lowercase().as_str() {
                "prev" => crate::dispatch::Direction::Prev,
                "next" => crate::dispatch::Direction::Next,
                other => {
                    warn!("truss: unknown focus direction '{other}', defaulting to next");
                    crate::dispatch::Direction::Next
                }
            };
            let cmd = Command::WindowFocusDir { direction };
            lua.to_value(&cmd)
        })?;
        cmd_table.set("window_focus_dir", win_focus_dir)?;

        let swap_master = self.lua.create_function(|lua, ()| {
            let cmd = Command::WindowSwapMaster;
            lua.to_value(&cmd)
        })?;
        cmd_table.set("swap_master", swap_master)?;

        let close_win = self.lua.create_function(|lua, id: Option<u64>| {
            let cmd = Command::WindowClose {
                id: id.map(WindowId),
            };
            lua.to_value(&cmd)
        })?;
        cmd_table.set("close_window", close_win)?;

        let toggle_float = self.lua.create_function(|lua, id: Option<u64>| {
            let cmd = Command::WindowToggleFloating {
                id: id.map(WindowId),
            };
            lua.to_value(&cmd)
        })?;
        cmd_table.set("toggle_floating", toggle_float)?;

        let toggle_fs = self.lua.create_function(|lua, id: Option<u64>| {
            let cmd = Command::WindowToggleFullscreen {
                id: id.map(WindowId),
            };
            lua.to_value(&cmd)
        })?;
        cmd_table.set("toggle_fullscreen", toggle_fs)?;

        let cmd_spawn = self.lua.create_function(|lua, command: String| {
            let cmd = Command::Spawn { command };
            lua.to_value(&cmd)
        })?;
        cmd_table.set("spawn", cmd_spawn)?;

        let set_gap = self.lua.create_function(|lua, gap: u32| {
            let cmd = Command::LayoutSetGap { gap };
            lua.to_value(&cmd)
        })?;
        cmd_table.set("set_gap", set_gap)?;

        let set_ratio = self.lua.create_function(|lua, ratio: f32| {
            let cmd = Command::LayoutSetRatio { ratio };
            lua.to_value(&cmd)
        })?;
        cmd_table.set("set_ratio", set_ratio)?;

        let quit = self.lua.create_function(|lua, ()| {
            let cmd = Command::CompositorQuit;
            lua.to_value(&cmd)
        })?;
        cmd_table.set("quit", quit)?;

        truss.set("cmd", cmd_table)?;
        globals.set("truss", truss)?;

        Ok(())
    }

    /// Extract registered window rules into WindowRuleManager
    pub fn apply_rules_to_manager(&self, manager: &mut WindowRuleManager) {
        if let Ok(rules) = self.lua.named_registry_value::<mlua::Table>("_truss_rules") {
            for entry in rules.sequence_values::<mlua::Table>().flatten() {
                let name: String = entry.get("name").unwrap_or_else(|_| "unnamed".into());
                if let Ok(rule_table) = entry.get::<mlua::Table>("rule") {
                    let mut matcher = WindowRuleMatcher::default();
                    if let Ok(app_id) = rule_table.get::<String>("app_id") {
                        matcher.app_id = Some(app_id);
                    }
                    if let Ok(title) = rule_table.get::<String>("title") {
                        matcher.title = Some(title);
                    }

                    let mut action = WindowRuleAction::default();
                    if let Ok(floating) = rule_table.get::<bool>("floating") {
                        action.open_floating = Some(floating);
                    }
                    if let Ok(ws) = rule_table.get::<u32>("workspace") {
                        action.open_on_workspace = Some(ws);
                    }
                    if let Ok(fs) = rule_table.get::<bool>("fullscreen") {
                        action.open_fullscreen = Some(fs);
                    }

                    manager.add_rule(WindowRule::new(name, matcher, action));
                }
            }
        }
    }

    /// Convert registered `truss.keybind` entries into Keybindings.
    pub fn apply_keybindings(&self, kb: &mut Keybindings) {
        let Ok(entries) = self
            .lua
            .named_registry_value::<mlua::Table>("_truss_keybinds")
        else {
            return;
        };
        for entry in entries.sequence_values::<mlua::Table>().flatten() {
            let mods_str: String = match entry.get("mods") {
                Ok(m) => m,
                Err(e) => {
                    warn!("truss: keybind invalid mods: {e}");
                    continue;
                }
            };

            // Empty mods string -> bare binding with no modifiers.
            let mut mods = Modifiers::NONE;
            let mut valid = true;
            if !mods_str.trim().is_empty() {
                for tok in mods_str.split('+') {
                    match tok.trim().to_lowercase().as_str() {
                        "ctrl" => mods.ctrl = true,
                        "alt" => mods.alt = true,
                        "shift" => mods.shift = true,
                        "super" | "mod4" | "logo" => mods.logo = true,
                        other => {
                            warn!("truss: unknown modifier '{other}' in keybind");
                            valid = false;
                            break;
                        }
                    }
                }
            }
            if !valid {
                continue;
            }

            let key: String = match entry.get("key") {
                Ok(k) => k,
                Err(e) => {
                    warn!("truss: keybind invalid key: {e}");
                    continue;
                }
            };
            let Some(keysym) = crate::input::keybindings::keysym_from_name(&key) else {
                warn!("truss: unknown key '{key}' in keybind");
                continue;
            };

            let action: mlua::Value = match entry.get("action") {
                Ok(a) => a,
                Err(e) => {
                    warn!("truss: keybind missing action: {e}");
                    continue;
                }
            };
            let action = match action {
                mlua::Value::String(s) => KeyAction::Spawn(s.to_string_lossy()),
                other => match self.lua.from_value::<Command>(other) {
                    Ok(cmd) => KeyAction::Dispatch(cmd),
                    Err(e) => {
                        warn!("truss: invalid keybind action: {e}");
                        continue;
                    }
                },
            };

            kb.bind(KeyPattern::new(mods, keysym), action);
        }
    }

    /// Apply settings registered via `truss.set(...)` onto the compositor.
    pub fn apply_settings(
        &self,
        dispatcher: &mut crate::dispatch::Dispatcher,
        state: &mut crate::state::State,
        bg_color: &mut smithay::backend::renderer::Color32F,
        border_config: &mut crate::app::BorderConfig,
    ) {
        let Ok(settings) = self
            .lua
            .named_registry_value::<mlua::Table>("_truss_settings")
        else {
            return;
        };
        for pair in settings.pairs::<String, mlua::Value>() {
            let Ok((name, value)) = pair else { continue };
            match name.as_str() {
                "gap" => match self.lua.from_value::<u32>(value) {
                    Ok(gap) => {
                        if let Err(e) = dispatcher.dispatch(state, Command::LayoutSetGap { gap }) {
                            warn!("truss: failed to apply gap setting: {e}");
                        }
                    }
                    Err(e) => warn!("truss: invalid gap setting: {e}"),
                },
                "ratio" => match self.lua.from_value::<f32>(value) {
                    Ok(ratio) => {
                        if let Err(e) =
                            dispatcher.dispatch(state, Command::LayoutSetRatio { ratio })
                        {
                            warn!("truss: failed to apply ratio setting: {e}");
                        }
                    }
                    Err(e) => warn!("truss: invalid ratio setting: {e}"),
                },
                "bg_color" => match self.lua.from_value::<String>(value) {
                    Ok(s) => match parse_hex_color(&s) {
                        Some([r, g, b, _]) => {
                            *bg_color = smithay::backend::renderer::Color32F::new(r, g, b, 1.0);
                        }
                        None => warn!("truss: malformed bg_color '{s}'"),
                    },
                    Err(e) => warn!("truss: invalid bg_color setting: {e}"),
                },
                "border_width" | "border.width" => match self.lua.from_value::<u32>(value) {
                    Ok(w) => border_config.width = w,
                    Err(e) => warn!("truss: invalid border_width setting: {e}"),
                },
                "active_border_color" | "border.active" | "border.active_color" => {
                    match self.lua.from_value::<String>(value) {
                        Ok(s) => match parse_hex_color(&s) {
                            Some([r, g, b, a]) => {
                                border_config.active_color =
                                    smithay::backend::renderer::Color32F::new(r, g, b, a);
                            }
                            None => warn!("truss: malformed active_border_color '{s}'"),
                        },
                        Err(e) => warn!("truss: invalid active_border_color setting: {e}"),
                    }
                }
                "inactive_border_color" | "border.inactive" | "border.inactive_color" => {
                    match self.lua.from_value::<String>(value) {
                        Ok(s) => match parse_hex_color(&s) {
                            Some([r, g, b, a]) => {
                                border_config.inactive_color =
                                    smithay::backend::renderer::Color32F::new(r, g, b, a);
                            }
                            None => warn!("truss: malformed inactive_border_color '{s}'"),
                        },
                        Err(e) => warn!("truss: invalid inactive_border_color setting: {e}"),
                    }
                }
                other => warn!("truss: unknown setting '{other}'"),
            }
        }
    }

    /// Run all commands registered with `truss.spawn_at_startup`
    pub fn run_autostart_commands(&self, socket_name: &str) {
        if let Ok(autostart) = self
            .lua
            .named_registry_value::<mlua::Table>("_truss_autostart")
        {
            for cmd in autostart.sequence_values::<String>().flatten() {
                info!("truss: autostarting process: {cmd}");
                let _ = crate::process::spawn_wayland_command(&cmd, socket_name);
            }
        }
    }

    /// Dispatch an internal Event into registered Lua callbacks.
    pub fn handle_event(&self, event: &Event) {
        let event_name = match event {
            Event::WorkspaceSwitched { .. } => "workspace.switched",
            Event::WindowCreated { .. } => "window.created",
            Event::WindowDestroyed { .. } => "window.destroyed",
            Event::WindowFocused { .. } => "window.focused",
            Event::WindowMovedWorkspace { .. } => "window.moved_workspace",
            Event::WindowStateChanged { .. } => "window.state_changed",
            Event::LayoutChanged { .. } => "layout.changed",
            Event::LayoutConfigChanged { .. } => "layout.config_changed",
            Event::CompositorQuitting => "compositor.quitting",
        };

        if let Ok(hooks) = self.lua.named_registry_value::<mlua::Table>("_truss_hooks") {
            if let Ok(mlua::Value::Table(list)) = hooks.get::<mlua::Value>(event_name) {
                if let Ok(val) = self.lua.to_value(event) {
                    for func in list.sequence_values::<mlua::Function>().flatten() {
                        if let Err(e) = func.call::<()>(val.clone()) {
                            warn!("Lua callback error for {event_name}: {e}");
                        }
                    }
                }
            }
        }
    }

    /// Load and execute a Lua configuration string.
    pub fn load_string(&self, code: &str) -> Result<(), mlua::Error> {
        self.lua.load(code).exec()
    }

    /// Load and execute a configuration file from path.
    pub fn load_file(&self, path: impl AsRef<Path>) -> Result<(), mlua::Error> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            mlua::Error::RuntimeError(format!("Cannot open config {}: {e}", path.display()))
        })?;
        self.lua
            .load(&content)
            .set_name(path.to_string_lossy())
            .exec()
    }

    /// Candidate config file locations, in priority order:
    /// $XDG_CONFIG_HOME/truss/config.lua, ~/.config/truss/config.lua, /etc/xdg/truss/config.lua.
    pub fn default_config_candidates() -> Vec<PathBuf> {
        let mut candidates = Vec::new();
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                candidates.push(PathBuf::from(xdg).join("truss").join("config.lua"));
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            if !home.is_empty() {
                candidates.push(
                    PathBuf::from(home)
                        .join(".config")
                        .join("truss")
                        .join("config.lua"),
                );
            }
        }
        candidates.push(PathBuf::from("/etc/xdg/truss/config.lua"));
        candidates
    }

    /// Pure core of [`Self::default_user_config_path`]: resolves the path from
    /// explicit `xdg`/`home` values instead of the environment, so the logic is
    /// testable without mutating env vars.
    pub fn default_user_config_path_from(xdg: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
        if let Some(xdg) = xdg {
            if !xdg.is_empty() {
                return Some(PathBuf::from(xdg).join("truss").join("config.lua"));
            }
        }
        if let Some(home) = home {
            if !home.is_empty() {
                return Some(
                    PathBuf::from(home)
                        .join(".config")
                        .join("truss")
                        .join("config.lua"),
                );
            }
        }
        None
    }

    /// The user config path where `truss init-config` writes the default config:
    /// $XDG_CONFIG_HOME/truss/config.lua, or ~/.config/truss/config.lua.
    pub fn default_user_config_path() -> Option<PathBuf> {
        let xdg = std::env::var("XDG_CONFIG_HOME").ok();
        let home = std::env::var("HOME").ok();
        Self::default_user_config_path_from(xdg.as_deref(), home.as_deref())
    }

    /// Resolve where configuration comes from: an explicit CLI path wins
    /// unconditionally, then the first existing candidate, else the embedded default.
    pub fn resolve_config_source(cli_path: Option<&Path>, candidates: &[PathBuf]) -> ConfigSource {
        if let Some(path) = cli_path {
            return ConfigSource::File(path.to_path_buf());
        }
        for candidate in candidates {
            if candidate.exists() {
                return ConfigSource::File(candidate.clone());
            }
        }
        ConfigSource::Embedded
    }

    /// Apply custom configuration values and sync Lua plugins to layout registry.
    pub fn apply_to_dispatcher(&self, dispatcher: &mut crate::dispatch::Dispatcher) {
        let _ = LuaPluginManager::register_layout_api(&self.lua, dispatcher);
        LuaPluginManager::sync_lua_layouts(&self.lua, &mut dispatcher.layout_registry);
    }

    /// Sync layout plugins from Lua into layout registry
    pub fn sync_layout_plugins(&self, registry: &mut LayoutRegistry) {
        LuaPluginManager::sync_lua_layouts(&self.lua, registry);
    }

    /// Retrieve a global string/value for testing/debugging.
    pub fn get_global<T: mlua::FromLua>(&self, name: &str) -> Result<T, mlua::Error> {
        self.lua.globals().get(name)
    }
}

/// Parse a hex color string (`#rgb` or `#rrggbb`) into RGBA floats (0.0..=1.0).
fn parse_hex_color(s: &str) -> Option<[f32; 4]> {
    let hex = s.trim().strip_prefix('#')?;
    // `hex.len()` is a byte count; slicing below must never land mid-UTF-8-char.
    if !hex.is_ascii() {
        return None;
    }
    let (r, g, b) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };
    Some([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
}
