pub mod plugins;

use mlua::{Lua, LuaSerdeExt};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

pub use plugins::LuaPluginManager;

use crate::dispatch::{Command, Event};
use crate::layout::LayoutRegistry;
use crate::state::{WindowId, WindowRule, WindowRuleAction, WindowRuleManager, WindowRuleMatcher};

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

        // Command constructors
        let cmd_table = self.lua.create_table()?;

        let ws_switch = self.lua.create_function(|lua, id: u32| {
            let cmd = Command::WorkspaceSwitch { id };
            lua.to_value(&cmd)
        })?;
        cmd_table.set("workspace_switch", ws_switch)?;

        let win_focus_dir = self.lua.create_function(|lua, dir: String| {
            let direction = match dir.to_lowercase().as_str() {
                "prev" => crate::dispatch::Direction::Prev,
                _ => crate::dispatch::Direction::Next,
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

    /// Locate default config file in $XDG_CONFIG_HOME/truss/config.lua or ~/.config/truss/config.lua
    pub fn find_default_config_path() -> Option<PathBuf> {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            let p = PathBuf::from(xdg).join("truss").join("config.lua");
            if p.exists() {
                return Some(p);
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let p = PathBuf::from(home)
                .join(".config")
                .join("truss")
                .join("config.lua");
            if p.exists() {
                return Some(p);
            }
        }
        None
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
