use std::{sync::Arc, time::Duration};

use smithay::{
    backend::{
        input::{ButtonState, Event, InputEvent, KeyboardKeyEvent, PointerButtonEvent, PointerMotionAbsoluteEvent},
        renderer::gles::GlesRenderer,
        winit::{self, WinitEvent},
    },
    input::keyboard::FilterResult,
    reexports::{
        calloop::{
            generic::Generic,
            timer::{TimeoutAction, Timer},
            EventLoop, Interest, Mode, PostAction,
        },
        wayland_server::Display,
    },
};
use tracing::{info, warn};
use wayland_server::ListeningSocket;

use truss::{
    dispatch::Command,
    input::{Modifiers, PointerFocusTarget},
    protocols::compositor::ClientState,
    App,
};

const WAYLAND_SOCKET: &str = "truss-0";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let mut display: Display<App> = Display::new()?;
    let mut app = App::new(&mut display)?;

    let dh = display.handle();
    let mut listener_dh = dh.clone();

    let mut event_loop = EventLoop::<App>::try_new()?;
    let loop_handle = event_loop.handle();

    let listener = ListeningSocket::bind(WAYLAND_SOCKET)?;
    info!("truss: wayland compositor running live at WAYLAND_DISPLAY={WAYLAND_SOCKET}");

    // Accept new wayland clients
    loop_handle.insert_source(
        Generic::new(listener, Interest::READ, Mode::Level),
        move |_, listener, state: &mut App| {
            while let Some(stream) = listener.accept()? {
                let client = listener_dh.insert_client(stream, Arc::new(ClientState::default()))?;
                state.clients.push(client);
            }
            Ok(PostAction::Continue)
        },
    )?;

    // Try initializing Winit graphical host window & backend
    let winit_backend = match winit::init::<GlesRenderer>() {
        Ok((backend, winit_event_loop)) => {
            info!("truss: initialized Winit host window backend (interactive input & rendering active)");
            let window_size = backend.window_size();
            app.output_manager
                .create_default_output("WINIT-1", (window_size.w, window_size.h).into());

            let mut graphics_backend = backend;
            let mut current_modifiers = Modifiers::NONE;

            loop_handle.insert_source(winit_event_loop, move |event, _, state: &mut App| {
                match event {
                    WinitEvent::Input(InputEvent::Keyboard { event }) => {
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
                                    if let Some(action) =
                                        data.keybindings.match_action(current_modifiers, sym).cloned()
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
                    WinitEvent::Input(InputEvent::PointerMotionAbsolute { event }) => {
                        let output_area = state.output_manager.primary_usable_area();
                        let (w, h) = (output_area.width as i32, output_area.height as i32);
                        let pos = smithay::utils::Point::from((
                            event.x_transformed(w),
                            event.y_transformed(h),
                        ));

                        state.pointer_state.set_location(pos);
                        state.pointer_state.update_drag(&mut state.state);
                    }
                    WinitEvent::Input(InputEvent::PointerButton { event }) => {
                        let target = state.pointer_state.find_target_at_location(&state.state);
                        let btn = event.button_code();
                        let is_pressed = event.state() == ButtonState::Pressed;

                        if is_pressed {
                            if let PointerFocusTarget::Window(win_id) = target {
                                if current_modifiers.logo {
                                    if let Some(win) = state.state.windows.get(&win_id) {
                                        let geom = win.geometry;
                                        if btn == 0x110 {
                                            // Super + Left Click: Interactive Move Drag
                                            state.pointer_state.start_drag_move(win_id, geom);
                                        } else if btn == 0x111 {
                                            // Super + Right Click: Interactive Resize Drag
                                            state.pointer_state.start_drag_resize(win_id, geom);
                                        }
                                    }
                                } else {
                                    // Normal Click: Focus Window
                                    let _ = state.dispatcher.dispatch(
                                        &mut state.state,
                                        Command::WindowFocus { id: win_id },
                                    );
                                }
                            }
                        } else {
                            // Release Button: End Drag
                            state.pointer_state.end_drag();
                        }
                    }
                    WinitEvent::CloseRequested => {
                        state.quit();
                    }
                    WinitEvent::Redraw => {
                        let _ = graphics_backend.bind();
                        let _ = graphics_backend.submit(None);
                    }
                    _ => {}
                }
            })?;
            Some(())
        }
        Err(err) => {
            warn!("truss: running in headless/socket mode (winit not started: {err})");
            None
        }
    };

    if winit_backend.is_some() {
        info!("truss: Ready! Press Super+Enter inside the window to spawn terminal, or Super+Drag to move/resize");
    } else {
        info!(
            "truss: Ready for clients! Launch apps with `WAYLAND_DISPLAY={WAYLAND_SOCKET} <app>`"
        );
    }

    // Periodic IPC poll & layout refresh
    loop_handle.insert_source(
        Timer::from_duration(Duration::from_millis(16)),
        move |_, _, state: &mut App| {
            state
                .ipc
                .poll_and_dispatch(&mut state.state, &mut state.dispatcher);
            state.refresh_layout_and_space();
            TimeoutAction::ToDuration(Duration::from_millis(16))
        },
    )?;

    while app.is_running() {
        event_loop.dispatch(Duration::from_millis(10), &mut app)?;
        display.dispatch_clients(&mut app)?;
        display.flush_clients()?;
    }

    warn!("truss: shutting down cleanly");
    Ok(())
}
