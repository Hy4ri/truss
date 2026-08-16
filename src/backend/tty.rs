use smithay::{
    backend::{
        input::{
            ButtonState, Event, InputEvent, KeyboardKeyEvent, PointerButtonEvent,
            PointerMotionEvent,
        },
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{libseat::LibSeatSession, Event as SessionEvent, Session},
    },
    input::keyboard::FilterResult,
    reexports::{calloop::LoopHandle, input::Libinput},
};
use tracing::info;

use crate::{
    dispatch::Command,
    input::{Modifiers, PointerFocusTarget},
    App,
};

/// TTY / Direct Bare-Metal Session Manager using libseat and libinput
pub struct TtyBackend {
    pub session: LibSeatSession,
}

impl TtyBackend {
    /// Attempt to initialize direct TTY session with libseat and libinput.
    pub fn init(
        loop_handle: &LoopHandle<'static, App>,
        app: &mut App,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("truss: initializing TTY direct backend via libseat and libinput");

        let (session, notifier) =
            LibSeatSession::new().map_err(|e| format!("Failed to open libseat session: {e}"))?;

        let seat_name = session.seat();
        info!("truss: libseat bound to seat '{seat_name}'");

        let mut libinput_ctx =
            Libinput::new_with_udev(LibinputSessionInterface::from(session.clone()));
        libinput_ctx
            .udev_assign_seat(&seat_name)
            .map_err(|_| "Failed to assign seat to libinput context")?;

        let libinput_backend = LibinputInputBackend::new(libinput_ctx);

        // Register session state notifier (VT switching & pause/resume)
        loop_handle.insert_source(notifier, move |event, _, state: &mut App| match event {
            SessionEvent::PauseSession => {
                info!("truss: session paused (switching VT)");
            }
            SessionEvent::ActivateSession => {
                info!("truss: session activated (returned to VT)");
                state.refresh_layout_and_space();
            }
        })?;

        // Register libinput event source for keyboard, mouse, and touch events
        let mut current_modifiers = Modifiers::NONE;

        loop_handle.insert_source(
            libinput_backend,
            move |event, _, state: &mut App| match event {
                InputEvent::Keyboard { event } => {
                    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                    let time = event.time_msec();
                    let key_code = event.key_code();
                    let key_state = event.state();

                    if let Some(keyboard) = state.keyboard.clone() {
                        keyboard.input::<(), _>(
                            state,
                            key_code,
                            key_state,
                            serial,
                            time,
                            |data, mods, handle| {
                                current_modifiers = Modifiers {
                                    ctrl: mods.ctrl,
                                    alt: mods.alt,
                                    shift: mods.shift,
                                    logo: mods.logo,
                                };
                                let sym = handle.modified_sym().raw();
                                if let Some(action) = data
                                    .keybindings
                                    .match_action(current_modifiers, sym)
                                    .cloned()
                                {
                                    let _ = data.keybindings.execute_action(
                                        &action,
                                        &mut data.dispatcher,
                                        &mut data.state,
                                    );
                                    FilterResult::Intercept(())
                                } else {
                                    FilterResult::Forward
                                }
                            },
                        );
                    }
                }
                InputEvent::PointerMotion { event } => {
                    let delta = smithay::utils::Point::from((event.delta_x(), event.delta_y()));
                    let bounds = state.output_manager.primary_usable_area();
                    state.pointer_state.update_location(delta, bounds);
                    state.pointer_state.update_drag(&mut state.state);
                }
                InputEvent::PointerButton { event } => {
                    let target = state.pointer_state.find_target_at_location(&state.state);
                    let btn = event.button_code();
                    let is_pressed = event.state() == ButtonState::Pressed;

                    if is_pressed {
                        if let PointerFocusTarget::Window(win_id) = target {
                            if current_modifiers.logo {
                                if let Some(win) = state.state.windows.get(&win_id) {
                                    let geom = win.geometry;
                                    if btn == 0x110 {
                                        state.pointer_state.start_drag_move(win_id, geom);
                                    } else if btn == 0x111 {
                                        state.pointer_state.start_drag_resize(win_id, geom);
                                    }
                                }
                            } else {
                                let _ = state.dispatcher.dispatch(
                                    &mut state.state,
                                    Command::WindowFocus { id: win_id },
                                );
                            }
                        }
                    } else {
                        state.pointer_state.end_drag();
                    }
                }
                _ => {}
            },
        )?;

        // Ensure primary TTY output is registered
        let _tty_output = app
            .output_manager
            .create_default_output("TTY-DRM-1", (1920, 1080).into());
        info!("truss: registered default physical display output 'TTY-DRM-1'");

        Ok(Self { session })
    }
}
