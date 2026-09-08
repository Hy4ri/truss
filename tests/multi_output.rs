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
