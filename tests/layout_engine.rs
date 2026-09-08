use truss::dispatch::{Command, Dispatcher};
use truss::layout::{Layout, LayoutConfig, MasterLayout, MonocleLayout};
use truss::state::{Rect, State, WindowId};

#[test]
fn test_master_layout_single_window() {
    let layout = MasterLayout;
    let config = LayoutConfig {
        gap: 10,
        master_ratio: 0.5,
        master_count: 1,
        smart_gaps: false,
    };
    let usable_area = Rect::new(0, 0, 1920, 1080);
    let windows = vec![WindowId(1)];

    let res = layout.arrange(&windows, usable_area, &config);
    assert_eq!(res.len(), 1);
    assert_eq!(res[0].0, WindowId(1));
    assert_eq!(res[0].1, Rect::new(10, 10, 1900, 1060));

    // Smart gaps test: gap is 0 when 1 window
    let smart_config = LayoutConfig {
        gap: 10,
        master_ratio: 0.5,
        master_count: 1,
        smart_gaps: true,
    };
    let smart_res = layout.arrange(&windows, usable_area, &smart_config);
    assert_eq!(smart_res[0].1, Rect::new(0, 0, 1920, 1080));
}

#[test]
fn test_master_layout_two_windows() {
    let layout = MasterLayout;
    let config = LayoutConfig {
        gap: 10,
        master_ratio: 0.5,
        master_count: 1,
        smart_gaps: false,
    };
    let usable_area = Rect::new(0, 0, 1920, 1080);
    let windows = vec![WindowId(1), WindowId(2)];

    let res = layout.arrange(&windows, usable_area, &config);
    assert_eq!(res.len(), 2);

    // Available width = 1920 - 3*10 = 1890.
    // Master w = 1890 * 0.5 = 945.
    // Stack w = 1890 - 945 = 945.
    assert_eq!(res[0].0, WindowId(1));
    assert_eq!(res[0].1, Rect::new(10, 10, 945, 1060));

    assert_eq!(res[1].0, WindowId(2));
    assert_eq!(res[1].1, Rect::new(10 + 945 + 10, 10, 945, 1060));
}

#[test]
fn test_master_layout_three_windows_stack() {
    let layout = MasterLayout;
    let config = LayoutConfig {
        gap: 10,
        master_ratio: 0.6,
        master_count: 1,
        smart_gaps: false,
    };
    let usable_area = Rect::new(0, 0, 1000, 1000);
    let windows = vec![WindowId(1), WindowId(2), WindowId(3)];

    let res = layout.arrange(&windows, usable_area, &config);
    assert_eq!(res.len(), 3);

    // usable = 1000x1000, gap = 10
    // total_w = 1000 - 30 = 970
    // master_w = round(970 * 0.6) = 582
    // stack_w = 970 - 582 = 388
    assert_eq!(res[0].1, Rect::new(10, 10, 582, 980));

    // stack windows: 2 windows.
    // stack_h available = 1000 - 3*10 = 970
    // each stack_h = 970 / 2 = 485
    assert_eq!(res[1].1, Rect::new(602, 10, 388, 485));
    // second stack window takes remainder to bottom
    assert_eq!(res[2].1, Rect::new(602, 505, 388, 485));
}

#[test]
fn test_monocle_layout() {
    let layout = MonocleLayout;
    let config = LayoutConfig {
        gap: 8,
        master_ratio: 0.5,
        master_count: 1,
        smart_gaps: false,
    };
    let usable_area = Rect::new(0, 0, 1920, 1080);
    let windows = vec![WindowId(1), WindowId(2)];

    let res = layout.arrange(&windows, usable_area, &config);
    assert_eq!(res.len(), 2);
    assert_eq!(res[0].1, Rect::new(8, 8, 1904, 1064));
    assert_eq!(res[1].1, Rect::new(8, 8, 1904, 1064));

    // Smart gaps test with 1 window in Monocle
    let single = vec![WindowId(1)];
    let smart_cfg = LayoutConfig {
        gap: 8,
        master_ratio: 0.5,
        master_count: 1,
        smart_gaps: true,
    };
    let smart_res = layout.arrange(&single, usable_area, &smart_cfg);
    assert_eq!(smart_res[0].1, Rect::new(0, 0, 1920, 1080));
}

#[test]
fn test_dispatcher_recalculate_workspace_layout() {
    let mut state = State::new();
    let dispatcher = Dispatcher::new();

    let w1 = state.create_window(Some(1)).unwrap();
    let w2 = state.create_window(Some(1)).unwrap();

    let display_area = Rect::new(0, 0, 1920, 1080);
    dispatcher.recalculate_workspace_layout(&mut state, 1, display_area);

    let win1_geom = state.windows.get(&w1).unwrap().geometry;
    let win2_geom = state.windows.get(&w2).unwrap().geometry;

    assert!(win1_geom.width > 0 && win1_geom.height > 0);
    assert!(win2_geom.width > 0 && win2_geom.height > 0);
    assert_eq!(win1_geom.x, 8); // default gap 8
    assert_ne!(win1_geom.x, win2_geom.x); // win2 is in the stack column
}

#[test]
fn test_dispatcher_layout_commands() {
    let mut state = State::new();
    let mut dispatcher = Dispatcher::new();

    // Test changing gap and ratio via commands
    dispatcher
        .dispatch(&mut state, Command::LayoutSetGap { gap: 16 })
        .unwrap();
    assert_eq!(dispatcher.layout_config.gap, 16);

    dispatcher
        .dispatch(&mut state, Command::LayoutSetRatio { ratio: 0.7 })
        .unwrap();
    assert!((dispatcher.layout_config.master_ratio - 0.7).abs() < f32::EPSILON);

    dispatcher
        .dispatch(
            &mut state,
            Command::LayoutSet {
                layout: "monocle".into(),
            },
        )
        .unwrap();
    assert_eq!(state.active_workspace().layout, "monocle");
}

#[test]
fn test_floating_fullscreen_restore_geometry() {
    let mut state = State::new();
    let mut dispatcher = Dispatcher::new();

    let win_id = state.create_window(Some(1)).unwrap();
    let initial_floating_geom = Rect::new(100, 150, 600, 400);
    state
        .set_window_geometry(win_id, initial_floating_geom)
        .unwrap();

    // 1. Make window floating
    dispatcher
        .dispatch(
            &mut state,
            Command::WindowToggleFloating { id: Some(win_id) },
        )
        .unwrap();
    assert!(state.windows.get(&win_id).unwrap().floating);

    let usable_area = Rect::new(0, 30, 1920, 1050);
    let full_area = Rect::new(0, 0, 1920, 1080);
    dispatcher.recalculate_workspace_layout_with_full_area(&mut state, 1, usable_area, full_area);
    assert_eq!(
        state.windows.get(&win_id).unwrap().geometry,
        initial_floating_geom
    );

    // 2. Make window fullscreen
    dispatcher
        .dispatch(
            &mut state,
            Command::WindowToggleFullscreen { id: Some(win_id) },
        )
        .unwrap();
    assert!(state.windows.get(&win_id).unwrap().fullscreen);

    dispatcher.recalculate_workspace_layout_with_full_area(&mut state, 1, usable_area, full_area);
    assert_eq!(state.windows.get(&win_id).unwrap().geometry, full_area);

    // 3. Toggle fullscreen off (back to floating mode)
    dispatcher
        .dispatch(
            &mut state,
            Command::WindowToggleFullscreen { id: Some(win_id) },
        )
        .unwrap();
    assert!(!state.windows.get(&win_id).unwrap().fullscreen);
    assert!(state.windows.get(&win_id).unwrap().floating);

    dispatcher.recalculate_workspace_layout_with_full_area(&mut state, 1, usable_area, full_area);
    // Geometry must be restored to the pre-fullscreen floating geometry
    assert_eq!(
        state.windows.get(&win_id).unwrap().geometry,
        initial_floating_geom
    );
}

#[test]
fn test_floating_maximize_and_fullscreen_restore_geometry() {
    let mut state = State::new();
    let mut dispatcher = Dispatcher::new();

    let win_id = state.create_window(Some(1)).unwrap();
    let initial_floating_geom = Rect::new(80, 120, 500, 350);
    state
        .set_window_geometry(win_id, initial_floating_geom)
        .unwrap();

    dispatcher
        .dispatch(
            &mut state,
            Command::WindowToggleFloating { id: Some(win_id) },
        )
        .unwrap();

    let usable_area = Rect::new(0, 30, 1920, 1050);
    let full_area = Rect::new(0, 0, 1920, 1080);

    // Maximize floating window
    dispatcher
        .dispatch(
            &mut state,
            Command::WindowToggleMaximize { id: Some(win_id) },
        )
        .unwrap();
    dispatcher.recalculate_workspace_layout_with_full_area(&mut state, 1, usable_area, full_area);
    assert_eq!(state.windows.get(&win_id).unwrap().geometry, usable_area);

    // Un-maximize returns to initial floating geometry
    dispatcher
        .dispatch(
            &mut state,
            Command::WindowToggleMaximize { id: Some(win_id) },
        )
        .unwrap();
    dispatcher.recalculate_workspace_layout_with_full_area(&mut state, 1, usable_area, full_area);
    assert_eq!(
        state.windows.get(&win_id).unwrap().geometry,
        initial_floating_geom
    );
}
