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
