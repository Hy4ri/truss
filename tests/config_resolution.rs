use std::sync::Mutex;

use truss::config::{ConfigSource, LuaConfig};
use truss::dispatch::Command;
use truss::input::{KeyAction, Keybindings, Modifiers};
use truss::App;

/// App::new binds a fixed IPC socket path, so tests constructing an App must
/// run serially to avoid racing on the socket.
static APP_LOCK: Mutex<()> = Mutex::new(());

/// Create a unique temporary directory (no tempfile crate).
fn unique_temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "truss_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_resolve_cli_wins() {
    let dir = unique_temp_dir("resolve_cli");
    let cli_path = dir.join("cli.lua");
    let first = dir.join("first.lua");
    let second = dir.join("second.lua");
    std::fs::write(&cli_path, "cli = true").unwrap();
    std::fs::write(&first, "first = true").unwrap();
    std::fs::write(&second, "second = true").unwrap();

    let candidates = vec![first.clone(), second.clone()];

    // Explicit CLI path wins unconditionally, even if candidates exist.
    match LuaConfig::resolve_config_source(Some(&cli_path), &candidates) {
        ConfigSource::File(p) => assert_eq!(p, cli_path),
        ConfigSource::Embedded => panic!("expected CLI file source"),
    }

    // Without a CLI path, the first existing candidate wins.
    match LuaConfig::resolve_config_source(None, &candidates) {
        ConfigSource::File(p) => assert_eq!(p, first),
        ConfigSource::Embedded => panic!("expected candidate file source"),
    }

    // No CLI path and no candidates -> embedded default.
    match LuaConfig::resolve_config_source(None, &[]) {
        ConfigSource::Embedded => {}
        ConfigSource::File(p) => panic!("expected embedded source, got {p:?}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_default_config_candidates_order() {
    // resolve_config_source honors candidate order: the first existing file wins,
    // missing earlier candidates are skipped.
    let dir = unique_temp_dir("candidates_order");
    let missing = dir.join("missing.lua");
    let first = dir.join("first.lua");
    let second = dir.join("second.lua");
    std::fs::write(&first, "x = 1").unwrap();
    std::fs::write(&second, "x = 2").unwrap();

    let candidates = vec![missing, first.clone(), second.clone()];
    match LuaConfig::resolve_config_source(None, &candidates) {
        ConfigSource::File(p) => assert_eq!(p, first),
        ConfigSource::Embedded => panic!("expected first existing candidate"),
    }

    // None of the candidates exist -> embedded.
    let none_existing = vec![
        dir.join("nope1.lua"),
        dir.join("nope2.lua"),
        dir.join("nope3.lua"),
    ];
    assert!(matches!(
        LuaConfig::resolve_config_source(None, &none_existing),
        ConfigSource::Embedded
    ));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_keybind_mods_parsing() {
    // Lowercase and mixed-case modifier tokens both parse.
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r#"
        truss.keybind("super+shift", "q", truss.cmd.quit())
        truss.keybind("Ctrl+Alt", "t", truss.cmd.spawn("xterm"))
    "#,
    )
    .unwrap();
    let mut kb = Keybindings::new();
    cfg.apply_keybindings(&mut kb);

    let shift_super = Modifiers {
        ctrl: false,
        alt: false,
        shift: true,
        logo: true,
    };
    assert_eq!(
        kb.match_action(shift_super, 0x0071),
        Some(&KeyAction::Dispatch(Command::CompositorQuit))
    );

    let ctrl_alt = Modifiers {
        ctrl: true,
        alt: true,
        shift: false,
        logo: false,
    };
    assert_eq!(
        kb.match_action(ctrl_alt, 0x0074),
        Some(&KeyAction::Dispatch(Command::Spawn {
            command: "xterm".into()
        }))
    );
}

#[test]
fn test_settings_applied() {
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r##"
        truss.set("gap", 16)
        truss.set("bg_color", "#ff0000")
    "##,
    )
    .unwrap();

    let _guard = APP_LOCK.lock().unwrap();
    let mut display = smithay::reexports::wayland_server::Display::<App>::new().unwrap();
    let mut app = App::new(&mut display, "test.sock").unwrap();
    cfg.apply_settings(
        &mut app.dispatcher,
        &mut app.state,
        &mut app.bg_color,
        &mut app.border_config,
        &mut app.focus_mode,
        &mut app.keyboard_config,
        &mut app.auto_reload,
    );

    assert_eq!(app.dispatcher.layout_config.gap, 16);
    assert!((app.bg_color.r() - 1.0).abs() < 1e-6);
    assert!(app.bg_color.g().abs() < 1e-6);
    assert!(app.bg_color.b().abs() < 1e-6);
    assert_eq!(app.bg_color.a(), 1.0);
}

#[test]
fn test_border_settings_applied() {
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r##"
        truss.set("border_width", 4)
        truss.set("active_border_color", "#ff00ff")
        truss.set("inactive_border_color", "#00ff00")
    "##,
    )
    .unwrap();

    let _guard = APP_LOCK.lock().unwrap();
    let mut display = smithay::reexports::wayland_server::Display::<App>::new().unwrap();
    let mut app = App::new(&mut display, "test.sock").unwrap();
    cfg.apply_settings(
        &mut app.dispatcher,
        &mut app.state,
        &mut app.bg_color,
        &mut app.border_config,
        &mut app.focus_mode,
        &mut app.keyboard_config,
        &mut app.auto_reload,
    );

    assert_eq!(app.border_config.width, 4);
    assert!((app.border_config.active_color.r() - 1.0).abs() < 1e-6);
    assert!(app.border_config.active_color.g().abs() < 1e-6);
    assert!((app.border_config.active_color.b() - 1.0).abs() < 1e-6);
    assert!(app.border_config.inactive_color.r().abs() < 1e-6);
    assert!((app.border_config.inactive_color.g() - 1.0).abs() < 1e-6);
    assert!(app.border_config.inactive_color.b().abs() < 1e-6);
}

#[test]
fn test_invalid_setting_warns() {
    // Unknown settings must be ignored without panicking or mutating state.
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string("truss.set(\"nope\", 1)").unwrap();

    let _guard = APP_LOCK.lock().unwrap();
    let mut display = smithay::reexports::wayland_server::Display::<App>::new().unwrap();
    let mut app = App::new(&mut display, "test.sock").unwrap();
    cfg.apply_settings(
        &mut app.dispatcher,
        &mut app.state,
        &mut app.bg_color,
        &mut app.border_config,
        &mut app.focus_mode,
        &mut app.keyboard_config,
        &mut app.auto_reload,
    );

    assert_eq!(app.dispatcher.layout_config.gap, 8);
    assert!((app.bg_color.r() - 0.08).abs() < 1e-6);
    assert!((app.bg_color.b() - 0.10).abs() < 1e-6);
}

#[test]
fn test_keybind_invalid_entries_skipped() {
    // Invalid modifier tokens and unknown key names are skipped; valid entries
    // in the same config are still applied.
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r#"
        truss.keybind("SUPER+HYPER", "q", truss.cmd.quit())
        truss.keybind("SUPER", "NotAKey", truss.cmd.quit())
        truss.keybind("SUPER", "Return", truss.cmd.spawn("kitty"))
    "#,
    )
    .unwrap();
    let mut kb = Keybindings::new();
    cfg.apply_keybindings(&mut kb);

    assert_eq!(
        kb.match_action(Modifiers::SUPER, 0xff0d),
        Some(&KeyAction::Dispatch(Command::Spawn {
            command: "kitty".into()
        }))
    );
    assert_eq!(kb.match_action(Modifiers::SUPER, 0x0071), None);
}

#[test]
fn test_move_to_workspace_cmd() {
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r#"
        truss.keybind("SUPER+SHIFT", "4", truss.cmd.move_to_workspace(4))
    "#,
    )
    .unwrap();
    let mut kb = Keybindings::new();
    cfg.apply_keybindings(&mut kb);

    let action = kb
        .match_action(Modifiers::SUPER_SHIFT, 0x0034)
        .expect("binding missing");
    assert_eq!(
        action,
        &KeyAction::Dispatch(Command::WindowMoveToWorkspace {
            window_id: None,
            workspace_id: 4,
        })
    );
}

#[test]
fn test_bare_keybind_no_modifiers() {
    // An empty mods string binds with Modifiers::NONE (bare key).
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r#"
        truss.keybind("", "Return", truss.cmd.quit())
    "#,
    )
    .unwrap();
    let mut kb = Keybindings::new();
    cfg.apply_keybindings(&mut kb);

    assert_eq!(
        kb.match_action(Modifiers::NONE, 0xff0d),
        Some(&KeyAction::Dispatch(Command::CompositorQuit))
    );
    // A modifier-carrying press must NOT match the bare binding.
    assert_eq!(kb.match_action(Modifiers::SUPER, 0xff0d), None);
}

#[test]
fn test_parse_hex_color_unicode_safe() {
    // Non-ASCII hex colors must not panic (byte-length slicing) and must be
    // rejected, leaving bg_color at its default.
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r##"
        truss.set("bg_color", "#é1")
        truss.set("bg_color", "#一")
        truss.set("bg_color", "#aéaé")
    "##,
    )
    .unwrap();

    let _guard = APP_LOCK.lock().unwrap();
    let mut display = smithay::reexports::wayland_server::Display::<App>::new().unwrap();
    let mut app = App::new(&mut display, "test.sock").unwrap();
    cfg.apply_settings(
        &mut app.dispatcher,
        &mut app.state,
        &mut app.bg_color,
        &mut app.border_config,
        &mut app.focus_mode,
        &mut app.keyboard_config,
        &mut app.auto_reload,
    );

    assert!((app.bg_color.r() - 0.08).abs() < 1e-6);
    assert!((app.bg_color.g() - 0.08).abs() < 1e-6);
    assert!((app.bg_color.b() - 0.10).abs() < 1e-6);
    assert_eq!(app.bg_color.a(), 1.0);
}

#[test]
fn test_keysym_punctuation() {
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r#"
        truss.keybind("SUPER", "comma", truss.cmd.quit())
        truss.keybind("SUPER", "minus", truss.cmd.quit())
        truss.keybind("SUPER", "f13", truss.cmd.quit())
        truss.keybind("SUPER", "plus", truss.cmd.quit())
    "#,
    )
    .unwrap();
    let mut kb = Keybindings::new();
    cfg.apply_keybindings(&mut kb);

    let expected = Some(&KeyAction::Dispatch(Command::CompositorQuit));
    assert_eq!(kb.match_action(Modifiers::SUPER, 0x002c), expected); // comma
    assert_eq!(kb.match_action(Modifiers::SUPER, 0x002d), expected); // minus
    assert_eq!(kb.match_action(Modifiers::SUPER, 0xffca), expected); // F13
    assert_eq!(kb.match_action(Modifiers::SUPER, 0x002b), expected); // plus
}

#[test]
fn test_cli_nonexistent_path_still_resolves_file() {
    // An explicit CLI path wins unconditionally, even when it does not exist;
    // the embedded-default fallback happens at load time, not resolve time.
    let dir = unique_temp_dir("cli_missing");
    let missing = dir.join("does-not-exist.lua");
    match LuaConfig::resolve_config_source(Some(&missing), &[]) {
        ConfigSource::File(p) => assert_eq!(p, missing),
        ConfigSource::Embedded => panic!("expected CLI file source"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_focus_mode_settings_applied() {
    let _guard = APP_LOCK.lock().unwrap();

    // 1. Enum string "follow_mouse"
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(r#"truss.set("focus_mode", "follow_mouse")"#)
        .unwrap();
    let mut display = smithay::reexports::wayland_server::Display::<App>::new().unwrap();
    let mut app = App::new(&mut display, "test.sock").unwrap();
    assert_eq!(app.focus_mode, truss::FocusMode::Click);

    cfg.apply_settings(
        &mut app.dispatcher,
        &mut app.state,
        &mut app.bg_color,
        &mut app.border_config,
        &mut app.focus_mode,
        &mut app.keyboard_config,
        &mut app.auto_reload,
    );
    assert_eq!(app.focus_mode, truss::FocusMode::FollowMouse);

    // 2. Boolean "focus_follow_mouse" = false
    let cfg_bool = LuaConfig::new().unwrap();
    cfg_bool
        .load_string(r#"truss.set("focus_follow_mouse", false)"#)
        .unwrap();
    cfg_bool.apply_settings(
        &mut app.dispatcher,
        &mut app.state,
        &mut app.bg_color,
        &mut app.border_config,
        &mut app.focus_mode,
        &mut app.keyboard_config,
        &mut app.auto_reload,
    );
    assert_eq!(app.focus_mode, truss::FocusMode::Click);

    // 3. Boolean "focus_follow_mouse" = true
    let cfg_bool_true = LuaConfig::new().unwrap();
    cfg_bool_true
        .load_string(r#"truss.set("focus_follow_mouse", true)"#)
        .unwrap();
    cfg_bool_true.apply_settings(
        &mut app.dispatcher,
        &mut app.state,
        &mut app.bg_color,
        &mut app.border_config,
        &mut app.focus_mode,
        &mut app.keyboard_config,
        &mut app.auto_reload,
    );
    assert_eq!(app.focus_mode, truss::FocusMode::FollowMouse);
}

#[test]
fn test_auto_reload_setting_applied() {
    let _guard = APP_LOCK.lock().unwrap();
    let mut display = smithay::reexports::wayland_server::Display::<App>::new().unwrap();
    let mut app = App::new(&mut display, "test.sock").unwrap();
    assert!(app.auto_reload);

    // Disable via auto_reload boolean
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(r#"truss.set("auto_reload", false)"#)
        .unwrap();
    cfg.apply_settings(
        &mut app.dispatcher,
        &mut app.state,
        &mut app.bg_color,
        &mut app.border_config,
        &mut app.focus_mode,
        &mut app.keyboard_config,
        &mut app.auto_reload,
    );
    assert!(!app.auto_reload);

    // Re-enable via hot_reload alias
    let cfg2 = LuaConfig::new().unwrap();
    cfg2.load_string(r#"truss.set("hot_reload", true)"#)
        .unwrap();
    cfg2.apply_settings(
        &mut app.dispatcher,
        &mut app.state,
        &mut app.bg_color,
        &mut app.border_config,
        &mut app.focus_mode,
        &mut app.keyboard_config,
        &mut app.auto_reload,
    );
    assert!(app.auto_reload);
}

#[test]
fn test_keyboard_settings_applied() {
    let _guard = APP_LOCK.lock().unwrap();
    let mut display = smithay::reexports::wayland_server::Display::<App>::new().unwrap();
    let mut app = App::new(&mut display, "test.sock").unwrap();

    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r#"
        truss.set("kb_layout", "us,ara")
        truss.set("kb_options", "grp:alt_shift_toggle,caps:escape")
        truss.set("repeat_rate", 40)
        truss.set("repeat_delay", 250)
        truss.set("numlock_by_default", true)
    "#,
    )
    .unwrap();

    cfg.apply_settings(
        &mut app.dispatcher,
        &mut app.state,
        &mut app.bg_color,
        &mut app.border_config,
        &mut app.focus_mode,
        &mut app.keyboard_config,
        &mut app.auto_reload,
    );

    assert_eq!(app.keyboard_config.layout, "us,ara");
    assert_eq!(
        app.keyboard_config.options.as_deref(),
        Some("grp:alt_shift_toggle,caps:escape")
    );
    assert_eq!(app.keyboard_config.repeat_rate, 40);
    assert_eq!(app.keyboard_config.repeat_delay, 250);
    assert!(app.keyboard_config.numlock_by_default);

    // Test that applying to keyboard handle compiles and updates state without panicking
    app.update_keyboard_config();
}

#[test]
fn test_reload_config_lifecycle_and_error_recovery() {
    let _guard = APP_LOCK.lock().unwrap();
    let dir = unique_temp_dir("hot_reload");
    let cfg_file = dir.join("config.lua");

    std::fs::write(
        &cfg_file,
        r#"
        truss.set("gap", 12)
        truss.set("auto_reload", true)
        truss.keybind("SUPER", "q", truss.cmd.quit())
    "#,
    )
    .unwrap();

    let mut display = smithay::reexports::wayland_server::Display::<App>::new().unwrap();
    let mut app = App::new(&mut display, "test.sock").unwrap();
    app.config_path = Some(cfg_file.clone());

    // Initial reload loads the file
    assert!(app.reload_config().is_ok());
    assert_eq!(app.dispatcher.layout_config.gap, 12);
    assert!(app.auto_reload);

    // 2. Modify config on disk and reload
    std::fs::write(
        &cfg_file,
        r#"
        truss.set("gap", 24)
        truss.set("auto_reload", false)
    "#,
    )
    .unwrap();
    assert!(app.reload_config().is_ok());
    assert_eq!(app.dispatcher.layout_config.gap, 24);
    assert!(!app.auto_reload);

    // 3. Write invalid Lua syntax — reload fails, but previous working state is preserved!
    std::fs::write(&cfg_file, "this is definitely not valid lua !!!").unwrap();
    assert!(app.reload_config().is_err());
    // Existing working configuration remains active
    assert_eq!(app.dispatcher.layout_config.gap, 24);
    assert!(!app.auto_reload);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_workspace_and_focus_history_lua_bindings() {
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r#"
        truss.keybind("SUPER", "Tab", truss.cmd.workspace_next())
        truss.keybind("SUPER+SHIFT", "Tab", truss.cmd.workspace_prev())
        truss.keybind("SUPER", "grave", truss.cmd.workspace_previous())
        truss.keybind("ALT", "Tab", truss.cmd.focus_last_window())
    "#,
    )
    .unwrap();

    let mut kb = truss::Keybindings::new();
    cfg.apply_keybindings(&mut kb);

    let mut state = truss::State::new();
    let mut dispatcher = truss::Dispatcher::new();

    // SUPER+Tab -> workspace_next (0xff09 is Tab keysym)
    let super_mod = truss::Modifiers {
        logo: true,
        ..truss::Modifiers::NONE
    };
    let super_shift_mod = truss::Modifiers {
        logo: true,
        shift: true,
        ..truss::Modifiers::NONE
    };
    let alt_mod = truss::Modifiers {
        alt: true,
        ..truss::Modifiers::NONE
    };

    let action_next = kb.match_action(super_mod, 0xff09).unwrap();
    kb.execute_action(action_next, &mut dispatcher, &mut state)
        .unwrap();
    assert_eq!(state.active_workspace_id, 2);

    let action_prev = kb.match_action(super_shift_mod, 0xff09).unwrap();
    kb.execute_action(action_prev, &mut dispatcher, &mut state)
        .unwrap();
    assert_eq!(state.active_workspace_id, 1);

    // SUPER+grave -> workspace_previous (0x0060 is grave keysym)
    // First switch 1 -> 5
    state.switch_workspace(5).unwrap();
    assert_eq!(state.active_workspace_id, 5);
    assert_eq!(state.previous_workspace_id, Some(1));

    let action_hist = kb.match_action(super_mod, 0x0060).unwrap();
    kb.execute_action(action_hist, &mut dispatcher, &mut state)
        .unwrap();
    assert_eq!(state.active_workspace_id, 1);
    assert_eq!(state.previous_workspace_id, Some(5));

    // ALT+Tab -> focus_last_window
    let w1 = state.create_window(Some(1)).unwrap();
    let w2 = state.create_window(Some(1)).unwrap();
    state.focus_window(w1).unwrap();
    state.focus_window(w2).unwrap();
    assert_eq!(state.active_workspace().focused_window, Some(w2));

    let action_alt_tab = kb.match_action(alt_mod, 0xff09).unwrap();
    kb.execute_action(action_alt_tab, &mut dispatcher, &mut state)
        .unwrap();
    assert_eq!(state.active_workspace().focused_window, Some(w1));

    kb.execute_action(action_alt_tab, &mut dispatcher, &mut state)
        .unwrap();
    assert_eq!(state.active_workspace().focused_window, Some(w2));
}

#[test]
fn test_smart_borders_and_gaps_settings() {
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r#"
        truss.set("smart_borders", true)
        truss.set("smart_gaps", true)
    "#,
    )
    .unwrap();

    let mut state = truss::State::new();
    let mut dispatcher = truss::Dispatcher::new();
    let mut bg_color = smithay::backend::renderer::Color32F::new(0.0, 0.0, 0.0, 1.0);
    let mut border_config = truss::BorderConfig::default();
    let mut focus_mode = truss::FocusMode::default();
    let mut keyboard_config = truss::KeyboardConfig::default();
    let mut auto_reload = false;

    cfg.apply_settings(
        &mut dispatcher,
        &mut state,
        &mut bg_color,
        &mut border_config,
        &mut focus_mode,
        &mut keyboard_config,
        &mut auto_reload,
    );

    assert!(border_config.smart_borders);
    assert!(dispatcher.layout_config.smart_gaps);
}

#[test]
fn test_move_to_workspace_silent_binding() {
    let cfg = LuaConfig::new().unwrap();
    cfg.load_string(
        r#"
        truss.keybind("SUPER+CTRL", "3", truss.cmd.move_to_workspace_silent(3))
        truss.keybind("SUPER+SHIFT", "3", truss.cmd.move_to_workspace(3))
    "#,
    )
    .unwrap();

    let mut kb = truss::Keybindings::new();
    cfg.apply_keybindings(&mut kb);

    let mut state = truss::State::new();
    let mut dispatcher = truss::Dispatcher::new();

    let w1 = state.create_window(Some(1)).unwrap();
    state.focus_window(w1).unwrap();
    assert_eq!(state.active_workspace_id, 1);
    assert_eq!(state.windows.get(&w1).unwrap().workspace_id, 1);

    let super_ctrl_mod = truss::Modifiers {
        logo: true,
        ctrl: true,
        ..truss::Modifiers::NONE
    };

    // '3' is 0x0033
    let action = kb.match_action(super_ctrl_mod, 0x0033).unwrap();
    kb.execute_action(action, &mut dispatcher, &mut state)
        .unwrap();

    // Window moved to workspace 3, but active workspace stayed 1 (silent!)
    assert_eq!(state.windows.get(&w1).unwrap().workspace_id, 3);
    assert_eq!(state.active_workspace_id, 1);

    // Now test move_to_workspace (follows window)
    let w2 = state.create_window(Some(1)).unwrap();
    state.focus_window(w2).unwrap();
    let super_shift_mod = truss::Modifiers {
        logo: true,
        shift: true,
        ..truss::Modifiers::NONE
    };
    let action_follow = kb.match_action(super_shift_mod, 0x0033).unwrap();
    kb.execute_action(action_follow, &mut dispatcher, &mut state)
        .unwrap();
    assert_eq!(state.windows.get(&w2).unwrap().workspace_id, 3);
    assert_eq!(state.active_workspace_id, 3);
}

#[test]
fn test_config_watcher_detects_file_save() {
    use truss::config::ConfigWatcher;

    let dir = unique_temp_dir("watcher");
    let cfg_file = dir.join("config.lua");
    std::fs::write(&cfg_file, "truss.set('gap', 8)").unwrap();

    let mut watcher = ConfigWatcher::new(&cfg_file).unwrap();
    // Initially queue is empty
    assert!(!watcher.check_events());

    // Overwrite file
    std::fs::write(&cfg_file, "truss.set('gap', 16)").unwrap();

    // Inotify should catch the write
    assert!(watcher.check_events());

    let _ = std::fs::remove_dir_all(&dir);
}
