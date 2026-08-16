use truss::state::{
    Window, WindowId, WindowRule, WindowRuleAction, WindowRuleManager, WindowRuleMatcher,
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
