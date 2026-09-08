use smithay::utils::{Point, Size};
use truss::backend::OutputManager;

#[test]
fn test_multi_output_arrangement() {
    let mut mgr = OutputManager::new();

    // Output 1: 1920x1080 at (0, 0)
    let o1 = mgr.create_output(
        "eDP-1",
        Point::from((0, 0)),
        Size::from((1920, 1080)),
        60_000,
    );
    assert_eq!(o1.name(), "eDP-1");

    // Output 2: 2560x1440 at (1920, 0) - Right of Output 1
    let o2 = mgr.create_output(
        "HDMI-A-1",
        Point::from((1920, 0)),
        Size::from((2560, 1440)),
        144_000,
    );
    assert_eq!(o2.name(), "HDMI-A-1");

    assert_eq!(mgr.outputs.len(), 2);
}

#[test]
fn test_output_removal_and_lookup() {
    let mut mgr = OutputManager::new();
    mgr.create_default_output("DP-1", Size::from((1920, 1080)));
    mgr.create_default_output("DP-2", Size::from((1920, 1080)));

    assert!(mgr.find_output_by_name("DP-1").is_some());
    assert!(mgr.find_output_by_name("DP-3").is_none());

    let removed = mgr.remove_output("DP-1");
    assert!(removed);
    assert_eq!(mgr.outputs.len(), 1);
    assert!(mgr.find_output_by_name("DP-1").is_none());
}

#[test]
fn test_declarative_monitor_configuration() {
    let cfg = truss::config::LuaConfig::new().unwrap();
    cfg.load_string(
        r#"
        truss.monitor({
            output = "eDP-1",
            mode = "1920x1080@120.00",
            position = "0x0",
            scale = 1.25,
        })
        truss.monitor({
            output = "HDMI-A-1",
            mode = "2560x1440@60.00",
            position = "1920x0",
            scale = 1.0,
        })
    "#,
    )
    .unwrap();

    let mut mgr = OutputManager::new();
    cfg.apply_monitors_to_manager(&mut mgr);

    assert_eq!(mgr.monitor_configs.len(), 2);
    let edp = mgr.find_monitor_config("eDP-1").unwrap();
    assert_eq!(edp.scale, Some(1.25));
    assert_eq!(edp.position.as_deref(), Some("0x0"));

    let o1 = mgr.create_output(
        "eDP-1",
        Point::from((0, 0)),
        Size::from((1920, 1080)),
        60_000,
    );
    mgr.apply_monitor_config_to_output(&o1);
    assert!(matches!(
        o1.current_scale(),
        smithay::output::Scale::Fractional(s) if (s - 1.25).abs() < f64::EPSILON
    ));
}

#[test]
fn test_workspace_output_binding_and_move() {
    let cfg = truss::config::LuaConfig::new().unwrap();
    cfg.load_string(
        r#"
        truss.workspace_rule({ workspace = 1, monitor = "eDP-1" })
        truss.workspace_rule({ workspace = 2, monitor = "HDMI-A-1" })
        truss.keybind("SUPER+ALT", "1", truss.cmd.move_workspace_to_monitor("eDP-1"))
    "#,
    )
    .unwrap();

    let mut state = truss::State::new();
    cfg.apply_workspace_rules(&mut state);

    assert_eq!(
        state.workspaces.get(&1).unwrap().output.as_deref(),
        Some("eDP-1")
    );
    assert_eq!(
        state.workspaces.get(&2).unwrap().output.as_deref(),
        Some("HDMI-A-1")
    );

    let mut dispatcher = truss::Dispatcher::new();
    dispatcher
        .dispatch(
            &mut state,
            truss::dispatch::Command::WorkspaceMoveToMonitor {
                workspace_id: Some(1),
                monitor: "HDMI-A-1".into(),
            },
        )
        .unwrap();

    assert_eq!(
        state.workspaces.get(&1).unwrap().output.as_deref(),
        Some("HDMI-A-1")
    );
}

#[test]
fn test_tabbed_grouping_and_navigation() {
    let mut state = truss::State::new();
    let mut dispatcher = truss::Dispatcher::new();

    let w1 = state.create_window(Some(1)).unwrap();
    let w2 = state.create_window(Some(1)).unwrap();

    state.focus_window(w1).unwrap();
    dispatcher
        .dispatch(&mut state, truss::dispatch::Command::GroupToggle)
        .unwrap();

    let g1 = state.windows.get(&w1).unwrap().group_id;
    let g2 = state.windows.get(&w2).unwrap().group_id;
    assert!(g1.is_some());
    assert_eq!(g1, g2);

    dispatcher
        .dispatch(&mut state, truss::dispatch::Command::GroupNext)
        .unwrap();
    assert_eq!(state.active_workspace().focused_window, Some(w2));

    dispatcher
        .dispatch(&mut state, truss::dispatch::Command::GroupPrev)
        .unwrap();
    assert_eq!(state.active_workspace().focused_window, Some(w1));

    // Dissolve w1 from group
    dispatcher
        .dispatch(&mut state, truss::dispatch::Command::GroupToggle)
        .unwrap();
    assert!(state.windows.get(&w1).unwrap().group_id.is_none());
}
