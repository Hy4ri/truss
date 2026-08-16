use smithay::utils::Size;
use truss::backend::{OutputManager, RenderManager, DESKTOP_BG_COLOR};
use truss::state::Rect;
use truss::App;

#[test]
fn test_output_manager_creation_and_usable_area() {
    let mut mgr = OutputManager::new();
    assert_eq!(mgr.outputs.len(), 0);

    let output = mgr.create_default_output("HDMI-A-1", Size::from((2560, 1440)));
    assert_eq!(mgr.outputs.len(), 1);
    assert_eq!(output.name(), "HDMI-A-1");

    let area = mgr.primary_usable_area();
    assert_eq!(area, Rect::new(0, 0, 2560, 1440));
}

#[test]
fn test_render_manager_space_initialization() {
    let render_mgr = RenderManager::new();
    assert_eq!(render_mgr.space.elements().count(), 0);
}

#[test]
fn test_desktop_bg_color() {
    assert_eq!(DESKTOP_BG_COLOR.a(), 1.0);
    assert!(DESKTOP_BG_COLOR.r() > 0.0);
    assert!(DESKTOP_BG_COLOR.b() > 0.0);
}

#[test]
fn test_app_refresh_layout_and_space() {
    let mut display = smithay::reexports::wayland_server::Display::<App>::new().unwrap();
    let mut app = App::new(&mut display).unwrap();

    let w1 = app.state.create_window(Some(1)).unwrap();
    let w2 = app.state.create_window(Some(1)).unwrap();

    app.refresh_layout_and_space();

    let win1_geom = app.state.windows.get(&w1).unwrap().geometry;
    let win2_geom = app.state.windows.get(&w2).unwrap().geometry;

    assert!(win1_geom.width > 0);
    assert!(win2_geom.width > 0);
    assert_ne!(win1_geom.x, win2_geom.x);
}
