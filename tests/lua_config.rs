use truss::config::LuaConfig;
use truss::dispatch::Event;
use truss::state::WindowId;

#[test]
fn test_lua_config_initialization() {
    let cfg = LuaConfig::new().expect("LuaConfig failed to initialize");
    cfg.load_string(
        r#"
        custom_gap = 24
        custom_name = "truss-test"
    "#,
    )
    .expect("Failed to execute Lua script");

    let gap: u32 = cfg.get_global("custom_gap").unwrap();
    let name: String = cfg.get_global("custom_name").unwrap();

    assert_eq!(gap, 24);
    assert_eq!(name, "truss-test");
}

#[test]
fn test_lua_truss_api_and_commands() {
    let cfg = LuaConfig::new().expect("LuaConfig failed to initialize");
    cfg.load_string(
        r#"
        version = truss.version
        ws_cmd = truss.cmd.workspace_switch(3)
        gap_cmd = truss.cmd.set_gap(16)
        ratio_cmd = truss.cmd.set_ratio(0.65)
    "#,
    )
    .expect("Failed to execute Lua script");

    let version: String = cfg.get_global("version").unwrap();
    assert!(!version.is_empty());
}

#[test]
fn test_lua_event_hooks() {
    let cfg = LuaConfig::new().expect("LuaConfig failed to initialize");
    cfg.load_string(
        r#"
        last_focused = 0
        truss.on("window.focused", function(ev)
            if ev.data and ev.data.id then
                last_focused = ev.data.id
            elseif ev.id then
                last_focused = ev.id
            end
        end)
    "#,
    )
    .expect("Failed to register on hook");

    let event = Event::WindowFocused { id: WindowId(42) };
    cfg.handle_event(&event);

    let focused_id: u32 = cfg.get_global("last_focused").unwrap();
    assert_eq!(focused_id, 42);
}

#[test]
fn test_lua_source_modular_file() {
    let cfg = LuaConfig::new().expect("LuaConfig failed to initialize");
    let tmp_dir = std::env::temp_dir();
    let sub_file = tmp_dir.join("truss_sub_config.lua");
    std::fs::write(&sub_file, "modular_loaded = true\nsub_val = 42").unwrap();

    let code = format!(r#"truss.source("{}")"#, sub_file.display());
    cfg.load_string(&code).expect("Failed to source Lua file");

    let loaded: bool = cfg.get_global("modular_loaded").unwrap();
    let val: u32 = cfg.get_global("sub_val").unwrap();

    assert!(loaded);
    assert_eq!(val, 42);

    let _ = std::fs::remove_file(sub_file);
}
