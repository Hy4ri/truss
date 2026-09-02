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
