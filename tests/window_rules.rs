use truss::state::{
    State, Window, WindowId, WindowRule, WindowRuleAction, WindowRuleManager, WindowRuleMatcher,
};
use truss::LuaConfig;

#[test]
fn test_window_rule_matching_and_apply() {
    let mut win = Window::new(WindowId(1), 1);
    win.app_id = Some("pavucontrol".into());
    win.title = Some("Volume Control".into());

    let matcher = WindowRuleMatcher {
        app_id: Some("pavucontrol".into()),
        title: None,
    };
    let action = WindowRuleAction {
        open_floating: Some(true),
        open_on_workspace: Some(5),
        open_fullscreen: None,
        center: None,
        pin: None,
        initial_size: None,
        initial_position: None,
        opacity: None,
    };

    let rule = WindowRule::new("float-audio", matcher, action);
    assert!(rule.apply(&mut win));
    assert!(win.floating);
    assert_eq!(win.workspace_id, 5);
}

#[test]
fn test_lua_window_rules_registration() {
    let cfg = LuaConfig::new().expect("Failed to initialize LuaConfig");
    cfg.load_string(
        r#"
        truss.window_rule("term-float", {
            app_id = "foot-float",
            floating = true,
            workspace = 3
        })
    "#,
    )
    .expect("Failed to load lua rule");

    let mut manager = WindowRuleManager::new();
    cfg.apply_rules_to_manager(&mut manager);

    let mut win = Window::new(WindowId(10), 1);
    win.app_id = Some("org.codeberg.dnkl.foot-float".into());
    manager.evaluate_and_apply(&mut win);

    assert!(win.floating);
    assert_eq!(win.workspace_id, 3);
}

#[test]
fn test_window_rule_center_and_syntax() {
    let cfg = LuaConfig::new().expect("Failed to initialize LuaConfig");
    cfg.load_string(
        r#"
        truss.window_rule({
            match = { class = "(pavucontrol|qalculate-gtk)" },
            float = true,
            center = true,
        })
    "#,
    )
    .expect("Failed to load lua rule");

    let mut manager = WindowRuleManager::new();
    cfg.apply_rules_to_manager(&mut manager);

    let mut win = Window::new(WindowId(20), 1);
    win.app_id = Some("org.pulseaudio.pavucontrol".into());
    manager.evaluate_and_apply(&mut win);

    assert!(win.floating);
    assert!(win.center);
}

#[test]
fn test_window_rule_pin_and_toggle_cmd() {
    let cfg = LuaConfig::new().expect("Failed to initialize LuaConfig");
    cfg.load_string(
        r#"
        truss.window_rule({
            match = { title = "Picture-in-Picture" },
            pin = true,
            float = true,
        })
    "#,
    )
    .expect("Failed to load lua rule");

    let mut manager = WindowRuleManager::new();
    cfg.apply_rules_to_manager(&mut manager);

    let mut win = Window::new(WindowId(33), 1);
    win.title = Some("Picture-in-Picture".into());
    manager.evaluate_and_apply(&mut win);

    assert!(win.pinned);
    assert!(win.floating);

    let mut state = State::new();
    let w_id = state.create_window(None).unwrap();
    assert!(!state.windows.get(&w_id).unwrap().pinned);
    state.toggle_pinned(w_id).unwrap();
    assert!(state.windows.get(&w_id).unwrap().pinned);
    state.toggle_pinned(w_id).unwrap();
    assert!(!state.windows.get(&w_id).unwrap().pinned);
}

#[test]
fn test_window_rule_initial_size_and_move() {
    let cfg = LuaConfig::new().expect("Failed to initialize LuaConfig");
    cfg.load_string(
        r#"
        truss.window_rule({
            match = { title = "PIP" },
            float = true,
            size = "500 400",
            ["move"] = "100 200",
        })
    "#,
    )
    .expect("Failed to load lua rule");

    let mut manager = WindowRuleManager::new();
    cfg.apply_rules_to_manager(&mut manager);

    let mut win = Window::new(WindowId(44), 1);
    win.title = Some("PIP".into());
    manager.evaluate_and_apply(&mut win);

    assert_eq!(win.initial_size.as_deref(), Some("500 400"));
    assert_eq!(win.initial_position.as_deref(), Some("100 200"));
}

#[test]
fn test_window_rule_opacity_and_settings() {
    let cfg = LuaConfig::new().expect("Failed to initialize LuaConfig");
    cfg.load_string(
        r#"
        truss.set("active_opacity", 0.95)
        truss.set("inactive_opacity", 0.80)
        truss.window_rule({
            match = { class = "Spotify" },
            opacity = 0.75,
        })
    "#,
    )
    .expect("Failed to load lua rule");

    let mut state = truss::State::new();
    let mut dispatcher = truss::Dispatcher::new();
    let mut bg_color = smithay::backend::renderer::Color32F::new(0.0, 0.0, 0.0, 1.0);
    let mut border_config = truss::BorderConfig::default();
    let mut opacity_config = truss::OpacityConfig::default();
    let mut focus_mode = truss::FocusMode::default();
    let mut keyboard_config = truss::KeyboardConfig::default();
    let mut auto_reload = false;

    cfg.apply_settings(
        &mut dispatcher,
        &mut state,
        &mut bg_color,
        &mut border_config,
        &mut opacity_config,
        &mut focus_mode,
        &mut keyboard_config,
        &mut auto_reload,
    );

    assert!((opacity_config.active_opacity - 0.95).abs() < 1e-4);
    assert!((opacity_config.inactive_opacity - 0.80).abs() < 1e-4);

    let mut manager = WindowRuleManager::new();
    cfg.apply_rules_to_manager(&mut manager);

    let mut win = Window::new(WindowId(50), 1);
    win.app_id = Some("Spotify".into());
    manager.evaluate_and_apply(&mut win);

    assert_eq!(win.opacity, Some(75));
}
