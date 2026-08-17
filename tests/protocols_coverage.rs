use smithay::reexports::wayland_server::Display;
use truss::App;

#[test]
fn test_protocols_initialization() {
    let mut display: Display<App> = Display::new().unwrap();
    let app = App::new(&mut display).unwrap();

    // Verify all core and desktop integration protocol states are active
    assert!(app.keyboard.is_some());
    assert!(app.pointer.is_some());
    assert_eq!(app.output_manager.outputs.len(), 1);
}
