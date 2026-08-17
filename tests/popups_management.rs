use smithay::reexports::wayland_server::Display;
use truss::App;

#[test]
fn test_popup_manager_initialization_and_cleanup() {
    let mut display: Display<App> = Display::new().unwrap();
    let mut app = App::new(&mut display).unwrap();
    // Test popup manager is initialized
    app.popups.cleanup();
}
