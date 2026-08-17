use smithay::reexports::wayland_server::Display;
use truss::App;

#[test]
fn test_window_focus_lifecycle() {
    let mut display: Display<App> = Display::new().unwrap();
    let mut app = App::new(&mut display).unwrap();

    // Create windows in state
    let win1 = app.state.create_window(None).unwrap();
    let win2 = app.state.create_window(None).unwrap();

    // Focus win1
    app.set_focused_window(Some(win1));
    assert_eq!(app.state.active_workspace().focused_window, Some(win1));

    // Focus win2
    app.set_focused_window(Some(win2));
    assert_eq!(app.state.active_workspace().focused_window, Some(win2));

    // Clear focus
    app.set_focused_window(None);
    assert_eq!(app.state.active_workspace().focused_window, None);
}
