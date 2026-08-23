use smithay::desktop::layer_map_for_output;
use smithay::reexports::wayland_server::Display;
use truss::App;

#[test]
fn test_layer_map_exclusive_zone_and_usable_area() {
    let mut display: Display<App> = Display::new().unwrap();
    let app = App::new(&mut display, "test.sock").unwrap();

    let usable_area = app.output_manager.primary_usable_area();
    assert_eq!(usable_area.width, 1920);
    assert_eq!(usable_area.height, 1080);

    // Verify layer map is reachable for primary output
    if let Some(output) = app.output_manager.outputs.first() {
        let layer_map = layer_map_for_output(output);
        let zone = layer_map.non_exclusive_zone();
        assert_eq!(zone.size.w, 1920);
        assert_eq!(zone.size.h, 1080);
    }
}
