use crate::layout::{Layout, LayoutConfig, LayoutRegistry};
use crate::state::{Rect, WindowId};

/// Thread-safe Layout trait object reference.
pub type BoxedLayout = Box<dyn Layout>;

/// Helper for building functional programmatic layouts.
pub struct FunctionalLayout<F>
where
    F: Fn(&[WindowId], Rect, &LayoutConfig) -> Vec<(WindowId, Rect)> + Send + Sync + 'static,
{
    pub name: String,
    pub func: F,
}

impl<F> Layout for FunctionalLayout<F>
where
    F: Fn(&[WindowId], Rect, &LayoutConfig) -> Vec<(WindowId, Rect)> + Send + Sync + 'static,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn arrange(
        &self,
        windows: &[WindowId],
        usable_area: Rect,
        config: &LayoutConfig,
    ) -> Vec<(WindowId, Rect)> {
        (self.func)(windows, usable_area, config)
    }
}

/// Lua Plugin Engine for registering layouts.
pub struct LuaPluginManager;

impl LuaPluginManager {
    /// Register layout extension points in Lua
    pub fn register_layout_api(
        _lua: &mlua::Lua,
        _dispatcher: &mut crate::dispatch::Dispatcher,
    ) -> Result<(), mlua::Error> {
        Ok(())
    }

    pub fn sync_lua_layouts(_lua: &mlua::Lua, _registry: &mut LayoutRegistry) {}
}
