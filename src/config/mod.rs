use mlua::{Lua, LuaSerdeExt};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::dispatch::{Command, Event};

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

    /// Dispatch an internal Event into registered Lua callbacks.
    pub fn handle_event(&self, event: &Event) {
        let event_name = match event {
            Event::WorkspaceSwitched { .. } => "workspace.switched",
            Event::WindowCreated { .. } => "window.created",
            Event::WindowDestroyed { .. } => "window.destroyed",
            Event::WindowFocused { .. } => "window.focused",
            Event::WindowMovedWorkspace { .. } => "window.moved_workspace",
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

    /// Evaluates user config or loads fallback default settings.
    pub fn apply_to_dispatcher(&self, dispatcher: &mut crate::dispatch::Dispatcher) {
        if let Ok(gap) = self.lua.globals().get::<u32>("gap") {
            dispatcher.layout_config.gap = gap;
            info!("Applied gap from config: {gap}px");
        }
        if let Ok(ratio) = self.lua.globals().get::<f32>("master_ratio") {
            dispatcher.layout_config.master_ratio = ratio.clamp(0.1, 0.9);
            info!("Applied master_ratio from config: {ratio}");
        }
    }

    /// Get evaluated global configuration value or fallback.
    pub fn get_global<T: mlua::FromLua>(&self, name: &str) -> Result<T, mlua::Error> {
        self.lua.globals().get(name)
    }
}
