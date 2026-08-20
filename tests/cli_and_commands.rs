use truss::cli::{write_default_config, CliArgs, Subcommand};
use truss::config::{LuaConfig, DEFAULT_CONFIG};
use truss::dispatch::{Command, DispatchResult, Dispatcher};
use truss::state::State;

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
fn test_cli_args_parsing_defaults() {
    let cli = CliArgs::default();
    assert_eq!(cli.socket_name, "truss-0");
    assert_eq!(cli.config_path, None);
    assert_eq!(cli.backend, None);
    assert_eq!(cli.subcommand, None);
}

#[test]
fn test_dispatch_window_close() {
    let mut state = State::new();
    let mut dispatcher = Dispatcher::new();

    let win_id = state.create_window(Some(1)).unwrap();
    assert_eq!(state.windows.len(), 1);

    let res = dispatcher
        .dispatch(&mut state, Command::WindowClose { id: Some(win_id) })
        .unwrap();
    assert_eq!(res, DispatchResult::Ok);
    assert_eq!(state.windows.len(), 0);
}

#[test]
fn test_dispatch_window_toggle_floating_and_fullscreen() {
    let mut state = State::new();
    let mut dispatcher = Dispatcher::new();

    let win_id = state.create_window(Some(1)).unwrap();
    assert!(!state.windows.get(&win_id).unwrap().floating);
    assert!(!state.windows.get(&win_id).unwrap().fullscreen);

    dispatcher
        .dispatch(
            &mut state,
            Command::WindowToggleFloating { id: Some(win_id) },
        )
        .unwrap();
    assert!(state.windows.get(&win_id).unwrap().floating);

    dispatcher
        .dispatch(
            &mut state,
            Command::WindowToggleFullscreen { id: Some(win_id) },
        )
        .unwrap();
    assert!(state.windows.get(&win_id).unwrap().fullscreen);
}

#[test]
fn test_cli_args_parsing_init_config() {
    let cli = CliArgs::parse_from(["init-config".to_string()].into_iter());
    assert_eq!(cli.subcommand, Some(Subcommand::InitConfig));
}

#[test]
fn test_write_default_config_creates_and_refuses_overwrite() {
    let dir = unique_temp_dir("init_config");
    // Nested target exercises parent directory creation.
    let target = dir.join("truss").join("config.lua");

    write_default_config(&target).unwrap();
    assert!(target.exists());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), DEFAULT_CONFIG);

    // A second write to the same path must be refused, leaving content intact.
    let err = write_default_config(&target).unwrap_err();
    assert!(err.contains("already exists"));
    assert_eq!(std::fs::read_to_string(&target).unwrap(), DEFAULT_CONFIG);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_default_user_config_path_from_home() {
    let path = LuaConfig::default_user_config_path_from(None, Some("/home/testuser")).unwrap();
    assert!(path.ends_with("truss/config.lua"));
    assert_eq!(
        path,
        std::path::PathBuf::from("/home/testuser/.config/truss/config.lua")
    );
}

#[test]
fn test_default_user_config_path_from_precedence() {
    // XDG wins over HOME when both are set and non-empty.
    let path =
        LuaConfig::default_user_config_path_from(Some("/custom/xdg"), Some("/home/u")).unwrap();
    assert_eq!(
        path,
        std::path::PathBuf::from("/custom/xdg/truss/config.lua")
    );

    // Empty strings are treated as unset (same guards as default_config_candidates).
    let path = LuaConfig::default_user_config_path_from(Some(""), Some("/home/u")).unwrap();
    assert_eq!(
        path,
        std::path::PathBuf::from("/home/u/.config/truss/config.lua")
    );

    assert_eq!(
        LuaConfig::default_user_config_path_from(Some(""), Some("")),
        None
    );
    assert_eq!(LuaConfig::default_user_config_path_from(None, None), None);
}
