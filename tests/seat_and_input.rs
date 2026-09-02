use smithay::utils::Point;
use truss::config::LuaConfig;
use truss::dispatch::{Command, Direction, DispatchResult, Dispatcher};
use truss::input::{
    parse_vt_switch, KeyAction, KeyPattern, Keybindings, Modifiers, PointerDragMode,
    PointerFocusTarget, PointerState,
};
use truss::state::{Rect, State};

#[test]
fn test_vt_switch_keysym_calculation() {
    let mods = Modifiers {
        ctrl: true,
        alt: true,
        shift: false,
        logo: false,
    };

    // 1. XKB XF86Switch_VT_1..12 keysyms (0x1008FE01..=0x1008FE0C)
    for vt in 1..=12 {
        let sym: u32 = 0x1008_fe01 + (vt - 1);
        let calculated_vt = parse_vt_switch(Modifiers::NONE, sym, sym, 0);
        assert_eq!(calculated_vt, Some(vt as i32));
    }

    // 2. Ctrl + Alt + KEY_F1 (0xffbe) to KEY_F12 (0xffc9)
    for vt in 1..=12 {
        let sym: u32 = 0xffbe + (vt - 1);
        let calculated_vt = parse_vt_switch(mods, sym, sym, 0);
        assert_eq!(calculated_vt, Some(vt as i32));
    }

    // 3. Ctrl + Alt + evdev keycodes (59..=68 for F1-F10, 87 for F11, 88 for F12)
    for vt in 1..=10 {
        let code = 59 + (vt - 1);
        let calculated_vt = parse_vt_switch(mods, 0, 0, code);
        assert_eq!(calculated_vt, Some(vt as i32));
    }
    assert_eq!(parse_vt_switch(mods, 0, 0, 87), Some(11));
    assert_eq!(parse_vt_switch(mods, 0, 0, 88), Some(12));

    // 4. Inactive modifiers should return None for standard F1 keysym or evdev keycode
    assert_eq!(parse_vt_switch(Modifiers::NONE, 0xffbe, 0xffbe, 59), None);
}

#[test]
fn test_default_keybindings_match() {
    // Default keybindings are config-driven: exercise the Lua path end-to-end.
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r#"
        truss.keybind("SUPER", "Return", truss.cmd.spawn("kitty"))
        truss.keybind("SUPER+SHIFT", "q", truss.cmd.quit())
        truss.keybind("SUPER", "j", truss.cmd.window_focus_dir("next"))
        truss.keybind("SUPER", "2", truss.cmd.workspace_switch(2))
    "#,
    )
    .unwrap();
    let mut kb = Keybindings::new();
    cfg.apply_keybindings(&mut kb);

    // Super + Return -> Spawn("kitty") via the Command::Spawn dispatch path
    let action_return = kb.match_action(Modifiers::SUPER, 0xff0d);
    assert_eq!(
        action_return,
        Some(&KeyAction::Dispatch(Command::Spawn {
            command: "kitty".into()
        }))
    );

    // Super + Shift + Q -> CompositorQuit (matching both lowercase 0x71 and uppercase 0x51)
    let action_quit_lower = kb.match_action(Modifiers::SUPER_SHIFT, 0x0071);
    assert_eq!(
        action_quit_lower,
        Some(&KeyAction::Dispatch(Command::CompositorQuit))
    );
    let action_quit_upper = kb.match_action(Modifiers::SUPER_SHIFT, 0x0051);
    assert_eq!(
        action_quit_upper,
        Some(&KeyAction::Dispatch(Command::CompositorQuit))
    );

    // Super + J / j -> WindowFocusDir(Next)
    let action_j_lower = kb.match_action(Modifiers::SUPER, 0x006a);
    assert_eq!(
        action_j_lower,
        Some(&KeyAction::Dispatch(Command::WindowFocusDir {
            direction: Direction::Next
        }))
    );
    let action_j_upper = kb.match_action(Modifiers::SUPER, 0x004a);
    assert_eq!(
        action_j_upper,
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

    // Stacking priority: floating window over tiled window
    // Make w2 overlap w1's area
    state.windows.get_mut(&w2).unwrap().geometry = Rect::new(0, 0, 960, 1080);
    // Both tiled: reverse order picks w2 (last in ws.windows)
    ptr.set_location(Point::from((200.0, 400.0)));
    assert_eq!(
        ptr.find_target_at_location(&state),
        PointerFocusTarget::Window(w2)
    );

    // Make w1 floating: now w1 takes priority over tiled w2 even though w2 is later in list
    state.windows.get_mut(&w1).unwrap().floating = true;
    assert_eq!(
        ptr.find_target_at_location(&state),
        PointerFocusTarget::Window(w1)
    );

    // Make w2 fullscreen: now w2 takes absolute priority over floating w1
    state.windows.get_mut(&w2).unwrap().fullscreen = true;
    assert_eq!(
        ptr.find_target_at_location(&state),
        PointerFocusTarget::Window(w2)
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
    // Tiled window does NOT automatically become floating
    assert!(!state.windows.get(&w1).unwrap().floating);

    ptr.end_drag();
    assert_eq!(ptr.drag, PointerDragMode::None);

    // Start resize drag at (200, 180)
    ptr.start_drag_resize(w1, state.windows.get(&w1).unwrap().geometry);
    ptr.set_location(Point::from((250.0, 280.0)));
    ptr.update_drag(&mut state);

    let resized_geom = state.windows.get(&w1).unwrap().geometry;
    assert_eq!(resized_geom.width, 850);
    assert_eq!(resized_geom.height, 700);
    assert!(!state.windows.get(&w1).unwrap().floating);

    // Floating windows stay floating when dragged
    state.windows.get_mut(&w1).unwrap().floating = true;
    ptr.start_drag_move(w1, state.windows.get(&w1).unwrap().geometry);
    ptr.set_location(Point::from((300.0, 300.0)));
    ptr.update_drag(&mut state);
    assert!(state.windows.get(&w1).unwrap().floating);
}

#[test]
fn test_tiled_window_snaps_back_after_drag_release() {
    let mut state = State::new();
    let dispatcher = Dispatcher::new();
    let mut ptr = PointerState::new();

    let w1 = state.create_window(Some(1)).unwrap();
    let w2 = state.create_window(Some(1)).unwrap();
    let usable_area = Rect::new(0, 0, 1920, 1080);
    dispatcher.recalculate_workspace_layout(&mut state, 1, usable_area);

    let initial_w1_geom = state.windows.get(&w1).unwrap().geometry;
    let initial_w2_geom = state.windows.get(&w2).unwrap().geometry;

    // Drag move w1
    ptr.set_location(Point::from((100.0, 100.0)));
    ptr.start_drag_move(w1, initial_w1_geom);
    ptr.set_location(Point::from((250.0, 300.0)));
    ptr.update_drag(&mut state);

    // Geometry moved during drag, but window did NOT float
    let dragged_geom = state.windows.get(&w1).unwrap().geometry;
    assert_ne!(dragged_geom, initial_w1_geom);
    assert!(!state.windows.get(&w1).unwrap().floating);

    // Release drag: since window is tiled, layout snaps it back
    ptr.end_drag();
    if !state.windows.get(&w1).unwrap().floating {
        dispatcher.recalculate_workspace_layout(&mut state, 1, usable_area);
    }
    assert_eq!(state.windows.get(&w1).unwrap().geometry, initial_w1_geom);
    assert_eq!(state.windows.get(&w2).unwrap().geometry, initial_w2_geom);

    // Now toggle w1 to floating
    state.toggle_floating(w1).unwrap();
    assert!(state.windows.get(&w1).unwrap().floating);

    // Drag move floating w1
    ptr.start_drag_move(w1, initial_w1_geom);
    ptr.set_location(Point::from((400.0, 500.0)));
    ptr.update_drag(&mut state);

    // Release drag: floating window retains its dragged geometry
    ptr.end_drag();
    if !state.windows.get(&w1).unwrap().floating {
        dispatcher.recalculate_workspace_layout(&mut state, 1, usable_area);
    }
    let final_geom = state.windows.get(&w1).unwrap().geometry;
    assert_ne!(final_geom, initial_w1_geom);
    assert!(state.windows.get(&w1).unwrap().floating);
}
