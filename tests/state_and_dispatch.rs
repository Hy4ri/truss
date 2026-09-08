#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use truss::{
        dispatch::{Command, DispatchResult, Dispatcher, Event},
        state::{State, WindowId},
    };

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

    #[test]
    fn test_dispatcher_rejects_invalid_window_and_layout_commands() {
        let mut state = State::new();
        let mut dispatcher = Dispatcher::new();

        let err = dispatcher
            .dispatch(
                &mut state,
                Command::WindowClose {
                    id: Some(WindowId(999)),
                },
            )
            .unwrap_err();
        assert!(err.to_string().contains("Window with id"));

        let err = dispatcher
            .dispatch(
                &mut state,
                Command::LayoutSet {
                    layout: "does-not-exist".into(),
                },
            )
            .unwrap_err();
        assert!(err.to_string().contains("Unknown layout"));
        assert_eq!(state.active_workspace().layout, "master");
    }

    #[test]
    fn test_window_state_changes_are_observable_and_gap_is_bounded() {
        let mut state = State::new();
        let mut dispatcher = Dispatcher::new();
        let window = state.create_window(None).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let recorded_events = events.clone();
        dispatcher.subscribe(move |event| recorded_events.lock().unwrap().push(event.clone()));

        dispatcher
            .dispatch(
                &mut state,
                Command::WindowToggleFloating { id: Some(window) },
            )
            .unwrap();
        dispatcher
            .dispatch(&mut state, Command::LayoutSetGap { gap: u32::MAX })
            .unwrap();

        assert_eq!(dispatcher.layout_config.gap, 4_096);
        assert_eq!(
            events.lock().unwrap()[0],
            Event::WindowStateChanged {
                id: window,
                floating: true,
                fullscreen: false,
                maximized: false,
            }
        );
    }

    #[test]
    fn test_workspace_cycling_next_and_prev() {
        let mut state = State::new();
        let mut dispatcher = Dispatcher::new();

        assert_eq!(state.active_workspace_id, 1);

        // Cycle next: 1 -> 2
        dispatcher
            .dispatch(&mut state, Command::WorkspaceNext)
            .unwrap();
        assert_eq!(state.active_workspace_id, 2);

        // Cycle prev: 2 -> 1
        dispatcher
            .dispatch(&mut state, Command::WorkspacePrev)
            .unwrap();
        assert_eq!(state.active_workspace_id, 1);

        // Wrap around backward: 1 -> 9
        dispatcher
            .dispatch(&mut state, Command::WorkspacePrev)
            .unwrap();
        assert_eq!(state.active_workspace_id, 9);

        // Wrap around forward: 9 -> 1
        dispatcher
            .dispatch(&mut state, Command::WorkspaceNext)
            .unwrap();
        assert_eq!(state.active_workspace_id, 1);
    }

    #[test]
    fn test_workspace_previous_history_toggle() {
        let mut state = State::new();
        let mut dispatcher = Dispatcher::new();

        assert_eq!(state.active_workspace_id, 1);
        assert_eq!(state.previous_workspace_id, None);

        // Switch 1 -> 4
        dispatcher
            .dispatch(&mut state, Command::WorkspaceSwitch { id: 4 })
            .unwrap();
        assert_eq!(state.active_workspace_id, 4);
        assert_eq!(state.previous_workspace_id, Some(1));

        // Switch to previous (back-and-forth): 4 -> 1
        dispatcher
            .dispatch(&mut state, Command::WorkspacePrevious)
            .unwrap();
        assert_eq!(state.active_workspace_id, 1);
        assert_eq!(state.previous_workspace_id, Some(4));

        // Switch to previous again: 1 -> 4
        dispatcher
            .dispatch(&mut state, Command::WorkspacePrevious)
            .unwrap();
        assert_eq!(state.active_workspace_id, 4);
        assert_eq!(state.previous_workspace_id, Some(1));
    }

    #[test]
    fn test_window_focus_last_history_toggle() {
        let mut state = State::new();
        let mut dispatcher = Dispatcher::new();

        let w1 = state.create_window(Some(1)).unwrap();
        let w2 = state.create_window(Some(1)).unwrap();

        // Focus w1 explicitly
        dispatcher
            .dispatch(&mut state, Command::WindowFocus { id: w1 })
            .unwrap();
        assert_eq!(state.active_workspace().focused_window, Some(w1));

        // Focus w2 explicitly
        dispatcher
            .dispatch(&mut state, Command::WindowFocus { id: w2 })
            .unwrap();
        assert_eq!(state.active_workspace().focused_window, Some(w2));

        // Focus last active window: w2 -> w1
        dispatcher
            .dispatch(&mut state, Command::WindowFocusLast)
            .unwrap();
        assert_eq!(state.active_workspace().focused_window, Some(w1));

        // Focus last active window again: w1 -> w2
        dispatcher
            .dispatch(&mut state, Command::WindowFocusLast)
            .unwrap();
        assert_eq!(state.active_workspace().focused_window, Some(w2));

        // Destroying w1 cleans up history
        state.remove_window(w1).unwrap();
        assert!(!state.focus_history.contains(&w1));
        assert_eq!(state.last_focused_window(Some(1)), None);
    }
}
