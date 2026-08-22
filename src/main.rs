use std::{io::IsTerminal, sync::Arc, time::Duration};

use smithay::{
    backend::{
        input::{
            AbsolutePositionEvent, ButtonState, Event, InputEvent, KeyState, KeyboardKeyEvent,
            PointerButtonEvent,
        },
        renderer::{gles::GlesRenderer, utils::draw_render_elements, Frame, Renderer},
        winit::{self, WinitEvent},
    },
    desktop::utils::send_frames_surface_tree,
    input::keyboard::FilterResult,
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, Mode, PostAction},
        wayland_server::Display,
    },
    utils::{Rectangle, Transform},
};
use tracing::{info, warn};
use wayland_server::ListeningSocket;

use truss::{
    backend::TtyBackend,
    bar::run_status_bar,
    cli::{handle_init_config_command, handle_msg_command, CliArgs, Subcommand},
    config::{ConfigSource, LuaConfig, DEFAULT_CONFIG},
    input::{Modifiers, PointerDragMode, PointerFocusTarget},
    protocols::compositor::ClientState,
    App,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = CliArgs::parse();

    match cli.subcommand {
        Some(Subcommand::Help) => {
            CliArgs::print_help();
            return Ok(());
        }
        Some(Subcommand::Version) => {
            CliArgs::print_version();
            return Ok(());
        }
        Some(Subcommand::Bar) => {
            return run_status_bar("truss.sock");
        }
        Some(Subcommand::Msg(args)) => {
            return handle_msg_command(&args);
        }
        Some(Subcommand::InitConfig) => {
            return handle_init_config_command();
        }
        None => {}
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let mut display: Display<App> = Display::new()?;
    let mut app = App::new(&mut display)?;

    // Resolve configuration: CLI path > XDG user config > /etc/xdg default > embedded default.
    let config_source = LuaConfig::resolve_config_source(
        cli.config_path.as_deref(),
        &LuaConfig::default_config_candidates(),
    );
    match &config_source {
        ConfigSource::File(path) => {
            info!("truss: loading configuration from {}", path.display());
            if let Err(e) = app.lua_config.load_file(path) {
                warn!("truss: config load failed: {e}");
                warn!("truss: falling back to embedded default configuration");
                if let Err(e) = app.lua_config.load_string(DEFAULT_CONFIG) {
                    warn!("truss: embedded default config failed: {e}");
                }
            }
        }
        ConfigSource::Embedded => {
            info!("truss: no config file found, using embedded default configuration");
            if let Err(e) = app.lua_config.load_string(DEFAULT_CONFIG) {
                warn!("truss: embedded default config failed: {e}");
            }
        }
    }
    app.lua_config.apply_rules_to_manager(&mut app.window_rules);
    app.lua_config.apply_to_dispatcher(&mut app.dispatcher);
    app.lua_config.apply_keybindings(&mut app.keybindings);
    app.lua_config
        .apply_settings(&mut app.dispatcher, &mut app.state, &mut app.bg_color);

    let dh = display.handle();
    let mut listener_dh = dh.clone();

    let mut event_loop = EventLoop::<App>::try_new()?;
    let loop_handle = event_loop.handle();

    let socket_name = cli.socket_name.as_str();
    std::env::set_var("WAYLAND_DISPLAY", socket_name);
    let listener = ListeningSocket::bind(socket_name)?;
    info!("truss: wayland compositor running live at WAYLAND_DISPLAY={socket_name}");

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

    // Register IPC server with calloop event reactor
    app.ipc.register_calloop_source(&loop_handle)?;

    // Trigger autostart applications configured in Lua
    app.lua_config.run_autostart_commands(socket_name);

    // Backend Selection: Check CLI override or Auto-Detect
    let force_backend = cli.backend.as_deref();
    let launched_from_tty = std::io::stdin().is_terminal();

    let (try_winit, try_tty) = match force_backend {
        Some("winit") => (true, false),
        Some("tty") => (false, true),
        Some("headless") => (false, false),
        _ => {
            // Auto mode:
            // - On a real TTY, prefer direct TTY backend first.
            // - Otherwise, prefer nested winit backend first.
            if launched_from_tty {
                (false, true)
            } else {
                (true, false)
            }
        }
    };

    let winit_init = if try_winit {
        winit::init::<GlesRenderer>().ok()
    } else {
        None
    };

    if let Some((mut backend, winit_event_loop)) = winit_init {
        info!("truss: running on Winit host window backend (nested graphical mode)");
        // Replace the phantom headless output with the real winit-backed output
        app.output_manager.remove_output("HEADLESS-1");
        let window_size = backend.window_size();
        let default_output = app
            .output_manager
            .create_default_output("WINIT-1", (window_size.w, window_size.h).into());
        // CRITICAL: advertise the output as a wl_output global, otherwise clients
        // see zero monitors ("no monitors available" in terminal apps).
        let _global = default_output.create_global::<App>(&dh);

        info!("truss: Ready! Spawn apps with SUPER+Return (or via config), or `WAYLAND_DISPLAY={socket_name} <app>`");

        let start_time = std::time::Instant::now();
        let mut current_modifiers = Modifiers::NONE;
        let mut cursor_manager = truss::backend::CursorManager::new();
        let output_for_winit = default_output.clone();

        loop_handle.insert_source(
            winit_event_loop,
            move |event, _, state: &mut App| match event {
                WinitEvent::Input(InputEvent::Keyboard { event }) => {
                    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                    let time = event.time_msec();
                    let key_code = event.key_code();
                    let key_state = event.state();
                    let is_press = key_state == KeyState::Pressed;

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

                                // Only match keybinds on key press, not release
                                if !is_press {
                                    return FilterResult::Forward;
                                }

                                let sym = handle.modified_sym().raw();
                                let raw_sym =
                                    handle.raw_syms().first().map(|s| s.raw()).unwrap_or(sym);
                                if let Some(action) = data
                                    .keybindings
                                    .match_action(current_modifiers, sym)
                                    .or_else(|| {
                                        data.keybindings.match_action(current_modifiers, raw_sym)
                                    })
                                    .cloned()
                                {
                                    let _ = data.keybindings.execute_action(
                                        &action,
                                        &mut data.dispatcher,
                                        &mut data.state,
                                    );
                                    let new_focus = data.state.active_workspace().focused_window;
                                    data.set_focused_window(new_focus);
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

                    // Send configure to resized window if resizing
                    if let PointerDragMode::Resize { window_id, .. } = state.pointer_state.drag {
                        if let Some(surface) = state.surfaces.get(&window_id) {
                            if let Some(win) = state.state.windows.get(&window_id) {
                                surface.with_pending_state(|s| {
                                    s.size = Some(
                                        (win.geometry.width as i32, win.geometry.height as i32)
                                            .into(),
                                    );
                                });
                                surface.send_configure();
                            }
                        }
                    }

                    // Forward motion to client surface under pointer across all layers
                    let surface_under = state.surface_under(pos);
                    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                    let time = event.time_msec();
                    if let Some(pointer) = state.seat.get_pointer() {
                        pointer.motion(
                            state,
                            surface_under,
                            &smithay::input::pointer::MotionEvent {
                                location: pos,
                                serial,
                                time,
                            },
                        );
                        pointer.frame(state);
                    }
                }
                WinitEvent::Input(InputEvent::PointerButton { event }) => {
                    let target = state.pointer_state.find_target_at_location(&state.state);
                    let btn = event.button_code();
                    let is_pressed = event.state() == ButtonState::Pressed;
                    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                    let time = event.time_msec();

                    if is_pressed {
                        if let PointerFocusTarget::Window(win_id) = target {
                            state.set_focused_window(Some(win_id));
                            if current_modifiers.logo {
                                if let Some(win) = state.state.windows.get(&win_id) {
                                    let geom = win.geometry;
                                    if btn == 0x110 {
                                        state.pointer_state.start_drag_move(win_id, geom);
                                    } else if btn == 0x111 {
                                        state.pointer_state.start_drag_resize(win_id, geom);
                                    }
                                }
                            } else if let Some(pointer) = state.seat.get_pointer() {
                                pointer.button(
                                    state,
                                    &smithay::input::pointer::ButtonEvent {
                                        button: btn,
                                        state: ButtonState::Pressed,
                                        serial,
                                        time,
                                    },
                                );
                                pointer.frame(state);
                            }
                        } else {
                            state.set_focused_window(None);
                            if let Some(pointer) = state.seat.get_pointer() {
                                pointer.button(
                                    state,
                                    &smithay::input::pointer::ButtonEvent {
                                        button: btn,
                                        state: ButtonState::Pressed,
                                        serial,
                                        time,
                                    },
                                );
                                pointer.frame(state);
                            }
                        }
                    } else {
                        state.pointer_state.end_drag();
                        if let Some(pointer) = state.seat.get_pointer() {
                            pointer.button(
                                state,
                                &smithay::input::pointer::ButtonEvent {
                                    button: btn,
                                    state: ButtonState::Released,
                                    serial,
                                    time,
                                },
                            );
                            pointer.frame(state);
                        }
                    }
                }
                WinitEvent::Input(InputEvent::PointerAxis { event }) => {
                    use smithay::backend::input::PointerAxisEvent;
                    use smithay::input::pointer::AxisFrame;

                    let mut frame = AxisFrame::new(event.time_msec()).source(event.source());
                    let horizontal_amount = event
                        .amount(smithay::backend::input::Axis::Horizontal)
                        .unwrap_or_else(|| {
                            event
                                .amount_v120(smithay::backend::input::Axis::Horizontal)
                                .unwrap_or(0.0)
                                * 15.0
                                / 120.0
                        });
                    let vertical_amount = event
                        .amount(smithay::backend::input::Axis::Vertical)
                        .unwrap_or_else(|| {
                            event
                                .amount_v120(smithay::backend::input::Axis::Vertical)
                                .unwrap_or(0.0)
                                * 15.0
                                / 120.0
                        });

                    let horizontal_amount_v120 =
                        event.amount_v120(smithay::backend::input::Axis::Horizontal);
                    let vertical_amount_v120 =
                        event.amount_v120(smithay::backend::input::Axis::Vertical);

                    if horizontal_amount != 0.0 || horizontal_amount_v120.is_some() {
                        let axis = smithay::backend::input::Axis::Horizontal;
                        frame = frame.value(axis, horizontal_amount);
                        if let Some(v120) = horizontal_amount_v120 {
                            frame = frame.v120(axis, v120 as i32);
                        }
                    }
                    if vertical_amount != 0.0 || vertical_amount_v120.is_some() {
                        let axis = smithay::backend::input::Axis::Vertical;
                        frame = frame.value(axis, vertical_amount);
                        if let Some(v120) = vertical_amount_v120 {
                            frame = frame.v120(axis, v120 as i32);
                        }
                    }

                    if let Some(pointer) = state.seat.get_pointer() {
                        pointer.axis(state, frame);
                        pointer.frame(state);
                    }
                }
                WinitEvent::Resized { size, .. } => {
                    let mode = smithay::output::Mode {
                        size: (size.w, size.h).into(),
                        refresh: 60_000,
                    };
                    output_for_winit.change_current_state(Some(mode), None, None, None);
                    output_for_winit.set_preferred(mode);
                    state.refresh_layout_and_space();
                }
                WinitEvent::CloseRequested => {
                    state.quit();
                }
                _ => {}
            },
        )?;

        while app.is_running() {
            let size = backend.window_size();
            let damage = Rectangle::from_size(size);
            {
                if let Ok((renderer, mut framebuffer)) = backend.bind() {
                    let elements = truss::backend::collect_render_elements(
                        &app,
                        renderer,
                        &mut cursor_manager,
                    );

                    if let Ok(mut frame) =
                        renderer.render(&mut framebuffer, size, Transform::Flipped180)
                    {
                        let _ = frame.clear(app.bg_color, &[damage]);
                        let _ = draw_render_elements(&mut frame, 1.0, &elements, &[damage]);
                        let _ = frame.finish();
                    }
                }
            }
            let _ = backend.submit(Some(&[damage]));

            let elapsed = start_time.elapsed();
            for surface in app.xdg_shell_state.toplevel_surfaces() {
                let is_on_inactive_ws = app
                    .surfaces
                    .iter()
                    .find(|(_, s)| s.wl_surface() == surface.wl_surface())
                    .and_then(|(id, _)| app.state.windows.get(id))
                    .map(|w| w.workspace_id != app.state.active_workspace_id)
                    .unwrap_or(false);

                if !is_on_inactive_ws {
                    send_frames_surface_tree(
                        surface.wl_surface(),
                        &default_output,
                        elapsed,
                        None,
                        |_, _| Some(default_output.clone()),
                    );
                }
            }

            event_loop.dispatch(Duration::from_millis(500), &mut app)?;
            app.process_pending_events();
            display.dispatch_clients(&mut app)?;
            display.flush_clients()?;
        }
    } else if try_tty {
        match TtyBackend::init(&loop_handle, &dh, &mut app) {
            Ok(mut tty_backend) => {
                info!("truss: running directly on TTY (libseat + libinput + DRM/KMS active)");
                info!("truss: Ready! Spawn apps with SUPER+Return (or via config), or `WAYLAND_DISPLAY={socket_name} <app>`");
                info!(
                    "truss: Press Ctrl+Alt+F1..F12 to switch VTs, or Super+Shift+Q to exit cleanly"
                );

                let start_time = std::time::Instant::now();

                while app.is_running() {
                    let elapsed = start_time.elapsed();

                    // Render active physical DRM displays
                    for drm_display in &mut tty_backend.drm_displays {
                        if let Err(e) = drm_display.render_frame(&app) {
                            tracing::trace!("truss: DRM render frame error: {e}");
                        }
                    }

                    // Dispatch Wayland frame callbacks across active outputs
                    for output in &app.output_manager.outputs {
                        for surface in app.xdg_shell_state.toplevel_surfaces() {
                            let is_on_inactive_ws = app
                                .surfaces
                                .iter()
                                .find(|(_, s)| s.wl_surface() == surface.wl_surface())
                                .and_then(|(id, _)| app.state.windows.get(id))
                                .map(|w| w.workspace_id != app.state.active_workspace_id)
                                .unwrap_or(false);

                            if !is_on_inactive_ws {
                                send_frames_surface_tree(
                                    surface.wl_surface(),
                                    output,
                                    elapsed,
                                    None,
                                    |_, _| Some(output.clone()),
                                );
                            }
                        }
                    }

                    // Sleep until a real event arrives (input, client, vblank,
                    // timer). Rendering is paced by DRM vblanks, not by polling.
                    event_loop.dispatch(Duration::from_millis(500), &mut app)?;
                    tty_backend.handle_vblanks();
                    app.process_pending_events();
                    display.dispatch_clients(&mut app)?;
                    display.flush_clients()?;
                }
            }
            Err(err) => {
                warn!("truss: TTY initialization skipped ({err}), falling back to headless socket mode");
                info!("truss: Ready for clients! Launch apps with `WAYLAND_DISPLAY={socket_name} <app>`");
                // Advertise the headless output so clients see a monitor
                if let Some(o) = app.output_manager.find_output_by_name("HEADLESS-1") {
                    let _g = o.create_global::<App>(&dh);
                }

                while app.is_running() {
                    event_loop.dispatch(Duration::from_millis(10), &mut app)?;
                    app.process_pending_events();
                    display.dispatch_clients(&mut app)?;
                    display.flush_clients()?;
                }
            }
        }
    } else {
        warn!("truss: running in forced headless mode");
        info!("truss: Ready for clients! Launch apps with `WAYLAND_DISPLAY={socket_name} <app>`");
        // Advertise the headless output so clients see a monitor
        if let Some(o) = app.output_manager.find_output_by_name("HEADLESS-1") {
            let _g = o.create_global::<App>(&dh);
        }

        while app.is_running() {
            event_loop.dispatch(Duration::from_millis(10), &mut app)?;
            app.process_pending_events();
            display.dispatch_clients(&mut app)?;
            display.flush_clients()?;
        }
    }

    warn!("truss: shutting down cleanly");
    Ok(())
}
