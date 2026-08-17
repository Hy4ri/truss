use truss::LuaConfig;

#[test]
fn test_lua_spawn_and_autostart_registration() {
    let cfg = LuaConfig::new().expect("Failed to initialize LuaConfig");
    cfg.load_string(
        r#"
        truss.spawn_at_startup("echo hello")
        truss.spawn_at_startup("waybar")
    "#,
    )
    .expect("Failed to load lua autostart snippet");

    // Test that commands can be invoked safely
    cfg.run_autostart_commands("truss-test-socket");
}
