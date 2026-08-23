use smithay::{
    backend::{
        input::{
            AbsolutePositionEvent, ButtonState, Event, InputEvent, KeyState, KeyboardKeyEvent,
            PointerButtonEvent, PointerMotionEvent,
        },
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{libseat::LibSeatSession, Event as SessionEvent, Session},
    },
    input::keyboard::FilterResult,
    reexports::{calloop::LoopHandle, input::Libinput, wayland_server::DisplayHandle},
};
use std::sync::mpsc::{channel, Receiver};
use tracing::info;

use crate::{
    backend::drm::{discover_and_init_drm_displays, DrmDisplay},
    input::{Modifiers, PointerDragMode, PointerFocusTarget},
    App,
};

/// TTY / Direct Bare-Metal Session Manager using libseat, libinput, and DRM/KMS
pub struct TtyBackend {
    pub session: LibSeatSession,
    pub drm_displays: Vec<DrmDisplay>,
    vblank_rx: Receiver<(usize, smithay::reexports::drm::control::crtc::Handle)>,
}

impl TtyBackend {
    /// Attempt to initialize direct TTY session with libseat and libinput.
    pub fn init(
        loop_handle: &LoopHandle<'static, App>,
        dh: &DisplayHandle,
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
                // DRM surfaces were torn down while the other VT owned the
                // console; flag it and let the TTY loop (which owns the
                // displays) reset GBM state + clear pending_frame so
                // rendering resumes instead of black-screening forever.
                state.vt_resume_pending = true;
                state.refresh_layout_and_space();
            }
        })?;

        // Register libinput event source for keyboard, mouse, and touch events
        let mut current_modifiers = Modifiers::NONE;
        let mut session_for_vt = session.clone();

        loop_handle.insert_source(
            libinput_backend,
            move |event, _, state: &mut App| match event {
                InputEvent::Keyboard { event } => {
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
                                let sym = handle.modified_sym().raw();
                                let raw_sym =
                                    handle.raw_syms().first().map(|s| s.raw()).unwrap_or(sym);

                                // VT Switching escape hatch: XF86Switch_VT_1..12, Ctrl+Alt+F1..12, or evdev keycodes
                                // (check on both press and release for safety, but only act on press)
                                if is_press {
                                    if let Some(vt) = crate::input::parse_vt_switch(
                                        current_modifiers,
                                        sym,
                                        raw_sym,
                                        key_code.into(),
                                    ) {
                                        info!("truss: switching to VT {vt} (Ctrl+Alt+F{vt})");
                                        if let Err(e) = session_for_vt.change_vt(vt) {
                                            tracing::warn!(
                                                "truss: failed to switch to VT {vt}: {e}"
                                            );
                                        }
                                        return FilterResult::Intercept(());
                                    }
                                }

                                // Only match keybinds on key press, not release
                                if !is_press {
                                    return FilterResult::Forward;
                                }

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
                InputEvent::PointerMotion { event } => {
                    let delta = smithay::utils::Point::from((event.delta_x(), event.delta_y()));
                    let bounds = state.output_manager.primary_usable_area();
                    state.pointer_state.update_location(delta, bounds);
                    state.pointer_state.update_drag(&mut state.state);
                    state.needs_redraw = true;

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
                    let pos = state.pointer_state.location;
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
                InputEvent::PointerMotionAbsolute { event } => {
                    // Tablets / spice virtio tablets report absolute screen
                    // coordinates. Map them onto the primary output's geometry.
                    let Some(output) = state.output_manager.outputs.first() else {
                        return;
                    };
                    let (o_w, o_h) = output
                        .current_mode()
                        .map(|m| (m.size.w as f64, m.size.h as f64))
                        .unwrap_or((1280.0, 800.0));
                    let pos_logical = event
                        .position_transformed(smithay::utils::Size::from((o_w as i32, o_h as i32)));
                    {
                        state.pointer_state.location = pos_logical;
                        state.pointer_state.update_drag(&mut state.state);
                        state.needs_redraw = true;

                        let pos = state.pointer_state.location;
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
                }
                InputEvent::PointerButton { event } => {
                    state.needs_redraw = true;
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
                InputEvent::PointerAxis { event } => {
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
                _ => {}
            },
        )?;

        // Initialize DRM/KMS hardware display scanout
        let mut session_for_drm = session.clone();
        let (vblank_tx, vblank_rx) = channel();
        let drm_displays =
            discover_and_init_drm_displays(&mut session_for_drm, loop_handle, dh, app, vblank_tx);

        // Replace the phantom headless output
        app.output_manager.remove_output("HEADLESS-1");

        if drm_displays.is_empty() {
            info!(
                "truss: no DRM display hardware found, using fallback virtual output 'TTY-DRM-1'"
            );
            let _tty_output = app
                .output_manager
                .create_default_output("TTY-DRM-1", (1920, 1080).into());
            let _tty_global = _tty_output.create_global::<App>(dh);
        } else {
            info!(
                "truss: registered {} physical DRM display(s) to OutputManager",
                drm_displays.len()
            );
        }

        Ok(Self {
            session,
            drm_displays,
            vblank_rx,
        })
    }

    /// Complete queued scanout only after the corresponding DRM vblank.
    pub fn handle_vblanks(&mut self) {
        while let Ok((card_id, crtc)) = self.vblank_rx.try_recv() {
            if let Some(display) = self.display_for_crtc_mut(card_id, crtc) {
                if let Err(error) = display.frame_submitted() {
                    tracing::warn!("truss: failed to complete DRM frame: {error}");
                }
                display.pending_frame = false;
            }
        }
    }

    /// True while any display still has a queued flip awaiting its vblank.
    pub fn has_pending_frames(&self) -> bool {
        self.drm_displays.iter().any(|d| d.pending_frame)
    }

    fn display_for_crtc_mut(
        &mut self,
        card_id: usize,
        crtc: smithay::reexports::drm::control::crtc::Handle,
    ) -> Option<&mut DrmDisplay> {
        self.drm_displays
            .iter_mut()
            .find(|display| display.card_id == card_id && display.crtc == crtc)
    }
}
