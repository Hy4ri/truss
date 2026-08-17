use truss::layout::{Layout, LayoutConfig};
use truss::state::{Rect, WindowId};

#[test]
fn test_layout_name_and_arrange_trait() {
    struct DummyLayout;
    impl Layout for DummyLayout {
        fn name(&self) -> &str {
            "dummy"
        }
        fn arrange(
            &self,
            windows: &[WindowId],
            usable_area: Rect,
            _config: &LayoutConfig,
        ) -> Vec<(WindowId, Rect)> {
            windows.iter().map(|&w| (w, usable_area)).collect()
        }
    }

    let l = DummyLayout;
    assert_eq!(l.name(), "dummy");
}
