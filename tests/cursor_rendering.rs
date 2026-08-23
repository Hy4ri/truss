use smithay::input::pointer::CursorImageStatus;
use smithay::reexports::wayland_server::Display;
use truss::backend::CursorManager;
use truss::App;

#[test]
fn test_cursor_manager_initialization_and_fallback() {
    let mut cursor_mgr = CursorManager::new();
    let (_buf, (xhot, yhot)) = cursor_mgr.get_or_load_cursor("default", 24);
    assert!(xhot >= 0);
    assert!(yhot >= 0);
}

#[test]
fn test_app_cursor_status_lifecycle() {
    let mut display: Display<App> = Display::new().unwrap();
    let app = App::new(&mut display, "test.sock").unwrap();
    assert_eq!(app.cursor_status, CursorImageStatus::default_named());
}
