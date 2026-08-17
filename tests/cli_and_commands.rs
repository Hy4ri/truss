use truss::cli::CliArgs;
use truss::dispatch::{Command, DispatchResult, Dispatcher};
use truss::state::State;

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
