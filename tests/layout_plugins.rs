use truss::dispatch::Dispatcher;
use truss::layout::{GridLayout, Layout, LayoutConfig};
use truss::state::{Rect, WindowId};

#[test]
fn test_grid_layout_calculation() {
    let grid = GridLayout;
    let config = LayoutConfig::default();
    let area = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };

    let wins = vec![WindowId(1), WindowId(2), WindowId(3), WindowId(4)];
    let res = grid.arrange(&wins, area, &config);

    assert_eq!(res.len(), 4);
    assert_eq!(res[0].0, WindowId(1));
    assert_eq!(res[1].0, WindowId(2));
}

#[test]
fn test_custom_layout_plugin_registration() {
    let mut dispatcher = Dispatcher::new();

    // Register a programmatic custom 2-column split layout plugin via API
    dispatcher.layout_registry.register_fn(
        "custom-split",
        |windows: &[WindowId], area: Rect, _cfg: &LayoutConfig| {
            if windows.is_empty() {
                return Vec::new();
            }
            let w = area.width / windows.len() as u32;
            windows
                .iter()
                .enumerate()
                .map(|(i, &id)| {
                    (
                        id,
                        Rect {
                            x: area.x + (i as i32 * w as i32),
                            y: area.y,
                            width: w,
                            height: area.height,
                        },
                    )
                })
                .collect()
        },
    );

    assert!(dispatcher.layout_registry.get("custom-split").is_some());

    let area = Rect {
        x: 0,
        y: 0,
        width: 1000,
        height: 1000,
    };
    let layout = dispatcher.layout_registry.get("custom-split").unwrap();
    let res = layout.arrange(
        &[WindowId(10), WindowId(20)],
        area,
        &LayoutConfig::default(),
    );

    assert_eq!(res.len(), 2);
    assert_eq!(res[0].1.width, 500);
    assert_eq!(res[1].1.width, 500);
}
