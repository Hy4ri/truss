use std::{sync::Arc, time::Duration};

use smithay::{
    backend::{
        input::{
            AbsolutePositionEvent, ButtonState, Event, InputEvent, KeyboardKeyEvent,
            PointerButtonEvent,
        },
        renderer::{
            element::{
                surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement},
                Kind,
            },
            gles::GlesRenderer,
            utils::draw_render_elements,
            Color32F, Frame, Renderer,
        },
        winit::{self, WinitEvent},
    },
    desktop::utils::send_frames_surface_tree,
    input::keyboard::FilterResult,
    reexports::{
        calloop::{
            generic::Generic,
            timer::{TimeoutAction, Timer},
            EventLoop, Interest, Mode, PostAction,
        },
        wayland_server::Display,
    },
    utils::{Rectangle, Transform},
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

    let winit_init = winit::init::<GlesRenderer>();

    if let Ok((mut backend, winit_event_loop)) = winit_init {
        info!(
            "truss: initialized Winit host window backend (interactive input & rendering active)"
        );
        let window_size = backend.window_size();
        let default_output = app
            .output_manager
            .create_default_output("WINIT-1", (window_size.w, window_size.h).into());

        info!("truss: Ready! Press Super+Enter inside the window to spawn terminal, or Super+Drag to move/resize");

        let start_time = std::time::Instant::now();
        let mut current_modifiers = Modifiers::NONE;

        loop_handle.insert_source(
            winit_event_loop,
            move |event, _, state: &mut App| match event {
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
                WinitEvent::CloseRequested => {
                    state.quit();
                }
                _ => {}
            },
        )?;

        while app.is_running() {
            // Render composited frame
            let size = backend.window_size();
            let damage = Rectangle::from_size(size);
            {
                if let Ok((renderer, mut framebuffer)) = backend.bind() {
                    let elements = app
                        .xdg_shell_state
                        .toplevel_surfaces()
                        .iter()
                        .flat_map(|surface| {
                            let win_geom = app
                                .surfaces
                                .iter()
                                .find(|(_, s)| s.wl_surface() == surface.wl_surface())
                                .and_then(|(id, _)| app.state.windows.get(id))
                                .map(|w| (w.geometry.x, w.geometry.y))
                                .unwrap_or((0, 0));

                            render_elements_from_surface_tree(
                                renderer,
                                surface.wl_surface(),
                                win_geom,
                                1.0,
                                1.0,
                                Kind::Unspecified,
                            )
                        })
                        .collect::<Vec<WaylandSurfaceRenderElement<GlesRenderer>>>();

                    if let Ok(mut frame) =
                        renderer.render(&mut framebuffer, size, Transform::Flipped180)
                    {
                        let _ = frame.clear(Color32F::new(0.08, 0.08, 0.10, 1.0), &[damage]);
                        let _ = draw_render_elements(&mut frame, 1.0, &elements, &[damage]);
                        let _ = frame.finish();
                    }
                }
            }
            let _ = backend.submit(Some(&[damage]));

            let elapsed = start_time.elapsed();
            for surface in app.xdg_shell_state.toplevel_surfaces() {
                send_frames_surface_tree(
                    surface.wl_surface(),
                    &default_output,
                    elapsed,
                    None,
                    |_, _| None,
                );
            }

            event_loop.dispatch(Duration::from_millis(5), &mut app)?;
            display.dispatch_clients(&mut app)?;
            display.flush_clients()?;
        }
    } else {
        warn!("truss: running in headless/socket mode (winit host window not available)");
        info!(
            "truss: Ready for clients! Launch apps with `WAYLAND_DISPLAY={WAYLAND_SOCKET} <app>`"
        );

        while app.is_running() {
            event_loop.dispatch(Duration::from_millis(10), &mut app)?;
            display.dispatch_clients(&mut app)?;
            display.flush_clients()?;
        }
    }

    warn!("truss: shutting down cleanly");
    Ok(())
}
