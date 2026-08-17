use smithay::reexports::wayland_server::Display;
use truss::App;

#[test]
fn test_fractional_scale_and_viewporter_globals() {
    let mut display: Display<App> = Display::new().unwrap();
    let app = App::new(&mut display).unwrap();

    // Verify fractional scale and viewporter states are initialized
    let _scale_state = &app.fractional_scale_manager_state;
    let _viewporter_state = &app.viewporter_state;
}
