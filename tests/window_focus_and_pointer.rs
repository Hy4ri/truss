use smithay::reexports::wayland_server::Display;
use std::sync::Mutex;
use truss::App;

static APP_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_window_focus_lifecycle() {
    let _guard = APP_LOCK.lock().unwrap();
    let sock = format!("focus_life_{}.sock", std::process::id());
    let _ = std::fs::remove_file(&sock);
    let mut display: Display<App> = Display::new().unwrap();
    let mut app = App::new(&mut display, &sock).unwrap();

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

#[test]
fn test_focus_follow_mouse_mode() {
    use smithay::utils::Point;
    use truss::state::Rect;
    use truss::FocusMode;

    let _guard = APP_LOCK.lock().unwrap();
    let sock = format!("focus_follow_{}.sock", std::process::id());
    let _ = std::fs::remove_file(&sock);
    let mut display: Display<App> = Display::new().unwrap();
    let mut app = App::new(&mut display, &sock).unwrap();

    let win1 = app.state.create_window(None).unwrap();
    let win2 = app.state.create_window(None).unwrap();

    // Make both windows floating so layout recalculations preserve their manual geometries
    app.state.windows.get_mut(&win1).unwrap().floating = true;
    app.state.windows.get_mut(&win2).unwrap().floating = true;
    app.state.windows.get_mut(&win1).unwrap().geometry = Rect::new(0, 0, 500, 500);
    app.state.windows.get_mut(&win2).unwrap().geometry = Rect::new(500, 0, 500, 500);

    // Initial focus on win1
    app.set_focused_window(Some(win1));
    assert_eq!(app.state.active_workspace().focused_window, Some(win1));

    // 1. In Click mode (default), moving pointer over win2 does NOT change focus
    assert_eq!(app.focus_mode, FocusMode::Click);
    app.pointer_state.set_location(Point::from((750.0, 250.0)));
    app.update_focus_on_pointer_motion();
    assert_eq!(app.state.active_workspace().focused_window, Some(win1));

    // 2. Switch to FollowMouse mode
    app.focus_mode = FocusMode::FollowMouse;
    app.update_focus_on_pointer_motion();
    // Now focused window switches to win2 because pointer is at (750, 250)
    assert_eq!(app.state.active_workspace().focused_window, Some(win2));

    // 3. Move pointer to background (outside both windows) -> sloppy focus keeps win2
    app.pointer_state
        .set_location(Point::from((2000.0, 2000.0)));
    app.update_focus_on_pointer_motion();
    assert_eq!(app.state.active_workspace().focused_window, Some(win2));

    // 4. Move pointer to win1 -> focus switches to win1
    app.pointer_state.set_location(Point::from((250.0, 250.0)));
    app.update_focus_on_pointer_motion();
    assert_eq!(app.state.active_workspace().focused_window, Some(win1));

    // 5. While dragging win1, moving pointer over win2 does NOT steal focus
    let geom1 = app.state.windows.get(&win1).unwrap().geometry;
    app.pointer_state.start_drag_move(win1, geom1);
    app.pointer_state.set_location(Point::from((750.0, 250.0)));
    app.update_focus_on_pointer_motion();
    assert_eq!(app.state.active_workspace().focused_window, Some(win1));
}

#[test]
fn test_focus_follow_mouse_tiled_windows() {
    use smithay::utils::Point;
    use truss::FocusMode;

    let _guard = APP_LOCK.lock().unwrap();
    let sock = format!("focus_tiled_{}.sock", std::process::id());
    let _ = std::fs::remove_file(&sock);
    let mut display: Display<App> = Display::new().unwrap();
    let mut app = App::new(&mut display, &sock).unwrap();

    let w1 = app.state.create_window(None).unwrap();
    let w2 = app.state.create_window(None).unwrap();

    // Default tiled arrangement on 1920x1080 master layout:
    // w1 (master) occupies left side, w2 (stack) occupies right side.
    app.set_focused_window(Some(w1));
    assert_eq!(app.state.active_workspace().focused_window, Some(w1));

    app.focus_mode = FocusMode::FollowMouse;

    // Moving over stack column (right side: x=1500) focuses w2
    app.pointer_state.set_location(Point::from((1500.0, 500.0)));
    app.update_focus_on_pointer_motion();
    assert_eq!(app.state.active_workspace().focused_window, Some(w2));

    // Moving back to master column (left side: x=300) focuses w1
    app.pointer_state.set_location(Point::from((300.0, 500.0)));
    app.update_focus_on_pointer_motion();
    assert_eq!(app.state.active_workspace().focused_window, Some(w1));
}
