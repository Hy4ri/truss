use smithay::utils::Point;
use truss::dispatch::{Command, Direction, DispatchResult, Dispatcher};
use truss::input::{
    KeyAction, KeyPattern, Keybindings, Modifiers, PointerDragMode, PointerFocusTarget,
    PointerState,
};
use truss::state::{Rect, State};

#[test]
fn test_default_keybindings_match() {
    let kb = Keybindings::new_default();

    // Super + Return -> Spawn("foot")
    let action_return = kb.match_action(Modifiers::SUPER, 0xff0d);
    assert_eq!(action_return, Some(&KeyAction::Spawn("foot".into())));

    // Super + Shift + Q -> CompositorQuit
    let action_quit = kb.match_action(Modifiers::SUPER_SHIFT, 0x0071);
    assert_eq!(
        action_quit,
        Some(&KeyAction::Dispatch(Command::CompositorQuit))
    );

    // Super + J -> WindowFocusDir(Next)
    let action_j = kb.match_action(Modifiers::SUPER, 0x006a);
    assert_eq!(
        action_j,
        Some(&KeyAction::Dispatch(Command::WindowFocusDir {
            direction: Direction::Next
        }))
    );

    // Super + 2 -> WorkspaceSwitch(2)
    let action_ws2 = kb.match_action(Modifiers::SUPER, 0x0032);
    assert_eq!(
        action_ws2,
        Some(&KeyAction::Dispatch(Command::WorkspaceSwitch { id: 2 }))
    );
}

#[test]
fn test_custom_keybinding_execution() {
    let mut kb = Keybindings::new();
    let mut state = State::new();
    let mut dispatcher = Dispatcher::new();

    let pattern = KeyPattern::new(Modifiers::SUPER, 0x0061); // Super + a
    let action = KeyAction::Dispatch(Command::WorkspaceSwitch { id: 5 });
    kb.bind(pattern.clone(), action);

    let matched = kb.match_action(Modifiers::SUPER, 0x0061).unwrap();
    let res = kb
        .execute_action(matched, &mut dispatcher, &mut state)
        .unwrap();

    assert_eq!(res, DispatchResult::Ok);
    assert_eq!(state.active_workspace_id, 5);
}

#[test]
fn test_pointer_location_update_and_clamping() {
    let mut ptr = PointerState::new();
    let bounds = Rect::new(0, 0, 1920, 1080);

    ptr.set_location(Point::from((100.0, 100.0)));
    assert_eq!(ptr.location.x, 100.0);
    assert_eq!(ptr.location.y, 100.0);

    // Update by delta
    ptr.update_location(Point::from((50.0, -20.0)), bounds);
    assert_eq!(ptr.location.x, 150.0);
    assert_eq!(ptr.location.y, 80.0);

    // Out of bounds clamping
    ptr.update_location(Point::from((5000.0, 5000.0)), bounds);
    assert_eq!(ptr.location.x, 1920.0);
    assert_eq!(ptr.location.y, 1080.0);
}

#[test]
fn test_pointer_find_window_target() {
    let mut state = State::new();
    let mut ptr = PointerState::new();

    let w1 = state.create_window(Some(1)).unwrap();
    let w2 = state.create_window(Some(1)).unwrap();

    // Assign mock geometries: w1 left (0..960), w2 right (960..1920)
    state.windows.get_mut(&w1).unwrap().geometry = Rect::new(0, 0, 960, 1080);
    state.windows.get_mut(&w2).unwrap().geometry = Rect::new(960, 0, 960, 1080);

    // Pointing inside w1
    ptr.set_location(Point::from((200.0, 400.0)));
    assert_eq!(
        ptr.find_target_at_location(&state),
        PointerFocusTarget::Window(w1)
    );

    // Pointing inside w2
    ptr.set_location(Point::from((1200.0, 500.0)));
    assert_eq!(
        ptr.find_target_at_location(&state),
        PointerFocusTarget::Window(w2)
    );

    // Pointing outside on blank area
    ptr.set_location(Point::from((2000.0, 2000.0)));
    assert_eq!(
        ptr.find_target_at_location(&state),
        PointerFocusTarget::Background
    );
}

#[test]
fn test_pointer_interactive_drag_and_resize() {
    let mut state = State::new();
    let mut ptr = PointerState::new();

    let w1 = state.create_window(Some(1)).unwrap();
    state.windows.get_mut(&w1).unwrap().geometry = Rect::new(100, 100, 800, 600);

    // Start move drag at (150, 150)
    ptr.set_location(Point::from((150.0, 150.0)));
    ptr.start_drag_move(w1, Rect::new(100, 100, 800, 600));

    // Move pointer by +50, +30
    ptr.set_location(Point::from((200.0, 180.0)));
    ptr.update_drag(&mut state);

    let moved_geom = state.windows.get(&w1).unwrap().geometry;
    assert_eq!(moved_geom.x, 150);
    assert_eq!(moved_geom.y, 130);
    assert!(state.windows.get(&w1).unwrap().floating);

    ptr.end_drag();
    assert_eq!(ptr.drag, PointerDragMode::None);

    // Start resize drag at (200, 180)
    ptr.start_drag_resize(w1, state.windows.get(&w1).unwrap().geometry);
    ptr.set_location(Point::from((250.0, 280.0)));
    ptr.update_drag(&mut state);

    let resized_geom = state.windows.get(&w1).unwrap().geometry;
    assert_eq!(resized_geom.width, 850);
    assert_eq!(resized_geom.height, 700);
}
