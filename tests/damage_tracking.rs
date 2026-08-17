use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::reexports::wayland_server::Display;
use truss::App;

#[test]
fn test_output_damage_tracker_initialization() {
    let mut display: Display<App> = Display::new().unwrap();
    let app = App::new(&mut display).unwrap();

    if let Some(output) = app.output_manager.outputs.first() {
        let _tracker = OutputDamageTracker::from_output(output);
    }
}
