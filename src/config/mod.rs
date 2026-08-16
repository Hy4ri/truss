use mlua::{Lua, LuaSerdeExt};
use std::path::{Path, PathBuf};

use crate::dispatch::Command;

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

    /// Get evaluated global configuration value or fallback.
    pub fn get_global<T: mlua::FromLua>(&self, name: &str) -> Result<T, mlua::Error> {
        self.lua.globals().get(name)
    }
}
