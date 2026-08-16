#[cfg(test)]
mod tests {
    use truss::{
        dispatch::{Command, Direction, DispatchResult, Dispatcher, Event},
        state::{State, WindowId},
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_state_workspace_creation_and_switch() {
        let mut state = State::new();
        assert_eq!(state.active_workspace_id, 1);
        assert_eq!(state.workspaces.len(), 9);

        state.switch_workspace(3).unwrap();
        assert_eq!(state.active_workspace_id, 3);
        assert_eq!(state.active_workspace().name, "3");
    }

    #[test]
    fn test_state_window_lifecycle() {
        let mut state = State::new();
        let win1 = state.create_window(Some(1)).unwrap();
        let win2 = state.create_window(Some(1)).unwrap();

        assert_eq!(state.windows.len(), 2);
        assert_eq!(state.active_workspace().windows, vec![win1, win2]);
        assert_eq!(state.active_workspace().focused_window, Some(win1));

        state.move_window_to_workspace(win1, 2).unwrap();
        assert_eq!(state.workspaces.get(&1).unwrap().windows, vec![win2]);
        assert_eq!(state.workspaces.get(&2).unwrap().windows, vec![win1]);

        state.remove_window(win2).unwrap();
        assert_eq!(state.windows.len(), 1);
        assert!(state.workspaces.get(&1).unwrap().windows.is_empty());
    }

    #[test]
    fn test_dispatcher_commands_and_events() {
        let mut state = State::new();
        let mut dispatcher = Dispatcher::new();
        let win = state.create_window(Some(1)).unwrap();

        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        dispatcher.subscribe(move |ev| {
            events_clone.lock().unwrap().push(ev.clone());
        });

        // Switch workspace command
        let res = dispatcher
            .dispatch(&mut state, Command::WorkspaceSwitch { id: 4 })
            .unwrap();
        assert_eq!(res, DispatchResult::Ok);
        assert_eq!(state.active_workspace_id, 4);

        // Window focus command
        dispatcher
            .dispatch(&mut state, Command::WindowFocus { id: win })
            .unwrap();
        assert_eq!(state.active_workspace_id, 1);
        assert_eq!(state.active_workspace().focused_window, Some(win));

        let recorded = events.lock().unwrap().clone();
        assert_eq!(
            recorded,
            vec![
                Event::WorkspaceSwitched { id: 4 },
                Event::WindowFocused { id: win }
            ]
        );
    }
}
