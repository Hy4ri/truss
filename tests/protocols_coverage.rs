use smithay::reexports::wayland_server::Display;
use truss::App;

#[test]
fn test_protocols_initialization() {
    let mut display: Display<App> = Display::new().unwrap();
    let mut app = App::new(&mut display).unwrap();

    // Verify all core and desktop integration protocol states are active
    assert!(app.keyboard.is_some());
    assert!(app.pointer.is_some());
    assert_eq!(app.output_manager.outputs.len(), 1);

    // Test surface_under on empty desktop returns None
    let hit = app.surface_under((100.0, 100.0).into());
    assert!(hit.is_none());

    // Test window creation and hit-testing
    let win_id = app.state.create_window(Some(1)).unwrap();
    app.refresh_layout_and_space();
    let win = app.state.windows.get(&win_id).unwrap();
    assert!(win.geometry.width > 0 && win.geometry.height > 0);
}
