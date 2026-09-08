use smithay::{
    desktop::{layer_map_for_output, PopupManager},
    input::{
        keyboard::{KeyboardHandle, XkbConfig},
        pointer::{CursorImageStatus, PointerHandle},
        Seat, SeatState,
    },
    reexports::wayland_server::{Client, Display},
    wayland::{
        compositor::CompositorState,
        fractional_scale::FractionalScaleManagerState,
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::{wlr_layer::WlrLayerShellState, xdg::XdgShellState},
        shm::ShmState,
        viewporter::ViewporterState,
    },
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
};
use tracing::warn;

use crate::{
    backend::{OutputManager, RenderManager, DESKTOP_BG_COLOR},
    config::LuaConfig,
    dispatch::{Dispatcher, Event},
    input::{Keybindings, PointerState},
    ipc::IpcServer,
    state::{State, WindowId, WindowRuleManager},
    sync::TransactionManager,
};

/// Configuration for window borders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusMode {
    #[default]
    Click,
    FollowMouse,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BorderConfig {
    pub width: u32,
    pub active_color: smithay::backend::renderer::Color32F,
    pub inactive_color: smithay::backend::renderer::Color32F,
    pub smart_borders: bool,
}

impl Default for BorderConfig {
    fn default() -> Self {
        Self {
            width: 2,
            active_color: smithay::backend::renderer::Color32F::new(0.502, 0.835, 0.824, 1.0), // #80d5d2
            inactive_color: smithay::backend::renderer::Color32F::new(0.20, 0.20, 0.20, 1.0), // #333333
            smart_borders: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardConfig {
    pub rules: String,
    pub model: String,
    pub layout: String,
    pub variant: String,
    pub options: Option<String>,
    pub repeat_rate: i32,
    pub repeat_delay: i32,
    pub numlock_by_default: bool,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            rules: String::new(),
            model: String::new(),
            layout: String::new(),
            variant: String::new(),
            options: None,
            repeat_rate: 25,
            repeat_delay: 200,
            numlock_by_default: false,
        }
    }
}

pub struct App {
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub layer_shell_state: WlrLayerShellState,
    pub shm_state: ShmState,
    pub data_device_state: DataDeviceState,
    pub output_manager_state: OutputManagerState,
    pub fractional_scale_manager_state: FractionalScaleManagerState,
    pub viewporter_state: ViewporterState,
    pub xdg_decoration_state: smithay::wayland::shell::xdg::decoration::XdgDecorationState,
    pub seat_state: SeatState<Self>,
    pub seat: Seat<Self>,
    pub keyboard: Option<KeyboardHandle<Self>>,
    pub pointer: Option<PointerHandle<Self>>,
    pub pointer_state: PointerState,
    pub cursor_status: CursorImageStatus,
    pub pending_focus_window: Option<WindowId>,
    pub keybindings: Keybindings,
    pub window_rules: WindowRuleManager,
    pub bg_color: smithay::backend::renderer::Color32F,
    pub border_config: BorderConfig,
    pub focus_mode: FocusMode,
    pub keyboard_config: KeyboardConfig,
    pub config_path: Option<std::path::PathBuf>,
    pub auto_reload: bool,
    pub output_manager: OutputManager,
    pub render_manager: RenderManager,
    pub lua_config: LuaConfig,
    pub clients: Vec<Client>,
    pub surfaces: HashMap<WindowId, smithay::wayland::shell::xdg::ToplevelSurface>,
    pub popups: PopupManager,
    pub transaction_manager: TransactionManager,
    pub state: State,
    pub dispatcher: Dispatcher,
    pub ipc: IpcServer,
    pub event_rx: Receiver<Event>,
    pub shutdown: Arc<AtomicBool>,
    /// Set whenever anything visible changed (windows, focus, cursor, client
    /// damage). The TTY loop renders only when this is set, then clears it —
    /// idle desktops must not queue page-flips at refresh rate (flicker/CPU).
    pub needs_redraw: bool,
    /// Set by the session notifier when the session (re)activates after a VT
    /// switch. The TTY loop — which owns the DRM displays the notifier cannot
    /// reach — consumes it and calls `DrmDisplay::reset_state()` on each
    /// display so rendering resumes instead of black-screening forever.
    pub vt_resume_pending: bool,
}

impl App {
    pub fn new(display: &mut Display<Self>, ipc_socket_name: &str) -> Result<Self, std::io::Error> {
        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let fractional_scale_manager_state = FractionalScaleManagerState::new::<Self>(&dh);
        let viewporter_state = ViewporterState::new::<Self>(&dh);
        let xdg_decoration_state =
            smithay::wayland::shell::xdg::decoration::XdgDecorationState::new::<Self>(&dh);

        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&dh, "seat-0");

        // Initialize keyboard & pointer on seat
        let keyboard = seat
            .add_keyboard(XkbConfig::default(), 200, 25)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let pointer = seat.add_pointer();

        let mut output_manager = OutputManager::new();
        let _headless = output_manager.create_default_output("HEADLESS-1", (1920, 1080).into());

        let render_manager = RenderManager::new();

        let lua_config = LuaConfig::new()
            .map_err(|e| std::io::Error::other(format!("Lua initialization failed: {e}")))?;

        let state = State::new();
        let mut dispatcher = Dispatcher::new();
        let window_rules = WindowRuleManager::new();

        let ipc = IpcServer::new(ipc_socket_name)?;
        ipc.setup_broadcaster(&mut dispatcher);

        let (event_tx, event_rx): (Sender<Event>, Receiver<Event>) = channel();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        dispatcher.subscribe(move |event| {
            if let crate::dispatch::Event::CompositorQuitting = event {
                shutdown_clone.store(true, Ordering::SeqCst);
            }
            let _ = event_tx.send(event.clone());
        });

        Ok(Self {
            compositor_state,
            xdg_shell_state,
            layer_shell_state,
            shm_state,
            data_device_state,
            output_manager_state,
            fractional_scale_manager_state,
            viewporter_state,
            xdg_decoration_state,
            seat_state,
            seat,
            keyboard: Some(keyboard),
            pointer: Some(pointer),
            pointer_state: PointerState::new(),
            cursor_status: CursorImageStatus::default_named(),
            pending_focus_window: None,
            keybindings: Keybindings::new(),
            window_rules,
            bg_color: DESKTOP_BG_COLOR,
            border_config: BorderConfig::default(),
            focus_mode: FocusMode::default(),
            keyboard_config: KeyboardConfig::default(),
            config_path: None,
            auto_reload: true,
            output_manager,
            render_manager,
            lua_config,
            clients: Vec::new(),
            surfaces: HashMap::new(),
            popups: PopupManager::default(),
            transaction_manager: TransactionManager::new(),
            state,
            dispatcher,
            ipc,
            event_rx,
            shutdown,
            needs_redraw: true,
            vt_resume_pending: false,
        })
    }

    /// Update focused window state, seat keyboard focus, and toplevel activation states.
    pub fn set_focused_window(&mut self, window_id: Option<WindowId>) {
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();

        if let Some(id) = window_id {
            let _ = self.state.focus_window(id);

            if let Some(surface) = self.surfaces.get(&id) {
                let wl_surf = surface.wl_surface().clone();
                if let Some(keyboard) = self.seat.get_keyboard() {
                    keyboard.set_focus(self, Some(wl_surf), serial);
                }
            }
        } else {
            self.state.active_workspace_mut().focused_window = None;
            if let Some(keyboard) = self.seat.get_keyboard() {
                keyboard.set_focus(self, None, serial);
            }
        }

        self.refresh_layout_and_space();
    }

    /// Update window focus on pointer motion if `focus_mode == FocusMode::FollowMouse`.
    pub fn update_focus_on_pointer_motion(&mut self) {
        if self.focus_mode != FocusMode::FollowMouse {
            return;
        }
        if !matches!(
            self.pointer_state.drag,
            crate::input::pointer::PointerDragMode::None
        ) {
            return;
        }

        // If an interactive layer surface (like an open launcher) is under pointer, do not divert focus
        let surface_under = self.surface_under(self.pointer_state.location);
        let layer_wants_kb = surface_under
            .as_ref()
            .map(|(s, _)| {
                smithay::wayland::compositor::with_states(s, |states| {
                    let mut cached = states
                        .cached_state
                        .get::<smithay::wayland::shell::wlr_layer::LayerSurfaceCachedState>(
                    );
                    match cached.current().keyboard_interactivity {
                        smithay::wayland::shell::wlr_layer::KeyboardInteractivity::Exclusive
                        | smithay::wayland::shell::wlr_layer::KeyboardInteractivity::OnDemand => {
                            true
                        }
                        smithay::wayland::shell::wlr_layer::KeyboardInteractivity::None => false,
                    }
                })
            })
            .unwrap_or(false);

        if layer_wants_kb {
            return;
        }

        let target = self.pointer_state.find_target_at_location(&self.state);
        if let crate::input::pointer::PointerFocusTarget::Window(win_id) = target {
            let current_focused = self.state.active_workspace().focused_window;
            if current_focused != Some(win_id) {
                self.set_focused_window(Some(win_id));
            }
        }
    }

    /// Apply matching rules and keep the state workspace indexes in sync.
    ///
    /// Rules operate on a `Window`, while `State` also stores a per-workspace
    /// window list. Moving the window through `State` after evaluating rules is
    /// therefore essential: changing only `Window::workspace_id` leaves a
    /// window visible on the wrong workspace.
    pub fn apply_window_rules(&mut self, window_id: WindowId) {
        let original_workspace = match self.state.windows.get(&window_id) {
            Some(window) => window.workspace_id,
            None => return,
        };

        if let Some(window) = self.state.windows.get_mut(&window_id) {
            self.window_rules.evaluate_and_apply(window);
        }

        let requested_workspace = match self.state.windows.get(&window_id) {
            Some(window) => window.workspace_id,
            None => return,
        };

        if requested_workspace == original_workspace {
            return;
        }

        if !self.state.workspaces.contains_key(&requested_workspace) {
            warn!(
                "Ignoring window rule for {:?}: workspace {} does not exist",
                window_id, requested_workspace
            );
            if let Some(window) = self.state.windows.get_mut(&window_id) {
                window.workspace_id = original_workspace;
            }
            return;
        }

        // `move_window_to_workspace` determines the source workspace from the
        // window itself, so restore it before performing the atomic move.
        if let Some(window) = self.state.windows.get_mut(&window_id) {
            window.workspace_id = original_workspace;
        }
        let _ = self
            .state
            .move_window_to_workspace(window_id, requested_workspace);
    }

    /// Apply current `keyboard_config` to the active keyboard handle.
    pub fn update_keyboard_config(&mut self) {
        let Some(keyboard) = self.keyboard.clone() else {
            return;
        };

        let rules = self.keyboard_config.rules.clone();
        let model = self.keyboard_config.model.clone();
        let layout = self.keyboard_config.layout.clone();
        let variant = self.keyboard_config.variant.clone();
        let options = self.keyboard_config.options.clone();
        let repeat_rate = self.keyboard_config.repeat_rate;
        let repeat_delay = self.keyboard_config.repeat_delay;
        let numlock_by_default = self.keyboard_config.numlock_by_default;

        let xkb_config = XkbConfig {
            rules: &rules,
            model: &model,
            layout: &layout,
            variant: &variant,
            options,
        };

        if let Err(e) = keyboard.set_xkb_config(self, xkb_config) {
            warn!("truss: failed to apply XKB keyboard configuration: {e}");
        } else {
            tracing::info!(
                "truss: applied keyboard configuration: layout='{layout}', options='{:?}'",
                self.keyboard_config.options
            );
        }

        keyboard.change_repeat_info(repeat_rate, repeat_delay);

        if numlock_by_default {
            let mut mods = keyboard.modifier_state();
            mods.num_lock = true;
            keyboard.set_modifier_state(mods);
        }
    }

    /// Reload configuration from `self.config_path` if available.
    pub fn reload_config(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(path) = self.config_path.clone() else {
            return Ok(());
        };

        let new_config = LuaConfig::new()?;
        new_config.load_file(&path)?;

        // Atomically replace configuration once successfully parsed and validated
        self.lua_config = new_config;
        self.window_rules.clear();
        self.lua_config
            .apply_rules_to_manager(&mut self.window_rules);
        self.keybindings.clear();
        self.lua_config.apply_keybindings(&mut self.keybindings);
        self.lua_config.apply_settings(
            &mut self.dispatcher,
            &mut self.state,
            &mut self.bg_color,
            &mut self.border_config,
            &mut self.focus_mode,
            &mut self.keyboard_config,
            &mut self.auto_reload,
        );
        self.update_keyboard_config();
        self.lua_config.apply_to_dispatcher(&mut self.dispatcher);
        self.refresh_layout_and_space();
        self.needs_redraw = true;

        tracing::info!(
            "truss: configuration reloaded successfully from {}",
            path.display()
        );
        Ok(())
    }

    /// Deliver dispatcher events to Lua hooks and handle compositor-level events.
    pub fn process_pending_events(&mut self) {
        let mut reload_requested = false;
        for event in self.event_rx.try_iter() {
            if let Event::ConfigReloadRequested = event {
                reload_requested = true;
            }
            self.lua_config.handle_event(&event);
        }
        if reload_requested {
            if let Err(e) = self.reload_config() {
                tracing::warn!("truss: configuration reload error: {e}");
            }
        }
    }

    /// Refresh and recalculate layouts for active workspaces across outputs.
    pub fn refresh_layout_and_space(&mut self) {
        self.needs_redraw = true;
        // Cleanup dead popup trees periodically
        self.popups.cleanup();

        // Prune expired sync transactions
        self.transaction_manager.prune_expired();

        let area = self.output_manager.primary_usable_area();
        let full_area = self.output_manager.primary_full_area();
        let active_ws = self.state.active_workspace_id;
        self.dispatcher.recalculate_workspace_layout_with_full_area(
            &mut self.state,
            active_ws,
            area,
            full_area,
        );
        self.render_manager
            .sync_windows(&self.state, &self.surfaces);

        // Update toplevel window surface states and configure sizes.
        // Change-detecting: a surface is only sent a configure when its
        // pending state actually differs. Idle-bar polling (`truss msg
        // get-state` at 2Hz) used to reconfigure EVERY toplevel twice per
        // request — endless client wakeups and toolkit re-renders.
        let focused = self.state.active_workspace().focused_window;
        let bounds_size = smithay::utils::Size::from((area.width as i32, area.height as i32));

        // Synchronized-resize: collect every window whose configured size
        // actually changed this pass. If any changed, register a transaction
        // covering all of them; the render loops withhold presentation until
        // each window commits its new framebuffer (fail-safe: 300ms).
        let mut resized: Vec<crate::state::WindowId> = Vec::new();
        for (&id, surface) in &self.surfaces {
            if let Some(win) = self.state.windows.get(&id) {
                let is_on_active_ws = win.workspace_id == active_ws;
                let is_active = is_on_active_ws && Some(id) == focused;

                let mut size_changed = false;
                let mut state_changed = false;
                surface.with_pending_state(|state| {
                    use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State;

                    let old_size = state.size;
                    let old_activated = state.states.contains(State::Activated);
                    let old_fullscreen = state.states.contains(State::Fullscreen);
                    let tiled_set = [
                        State::TiledLeft,
                        State::TiledRight,
                        State::TiledTop,
                        State::TiledBottom,
                    ];
                    let old_tiled_any = tiled_set.iter().any(|s| state.states.contains(*s));

                    if is_active {
                        state.states.set(State::Activated);
                    } else {
                        state.states.unset(State::Activated);
                    }

                    if win.fullscreen {
                        state.states.set(State::Fullscreen);
                    } else {
                        state.states.unset(State::Fullscreen);
                    }

                    if is_on_active_ws {
                        let new_size: smithay::utils::Size<i32, smithay::utils::Logical> =
                            (win.geometry.width as i32, win.geometry.height as i32).into();
                        if state.size != Some(new_size) {
                            size_changed = true;
                        }
                        state.size = Some(new_size);

                        if !win.floating {
                            state.states.set(State::TiledLeft);
                            state.states.set(State::TiledRight);
                            state.states.set(State::TiledTop);
                            state.states.set(State::TiledBottom);
                            state.bounds = Some(bounds_size);
                        } else {
                            state.states.unset(State::TiledLeft);
                            state.states.unset(State::TiledRight);
                            state.states.unset(State::TiledTop);
                            state.states.unset(State::TiledBottom);
                        }
                    } else {
                        state.states.unset(State::TiledLeft);
                        state.states.unset(State::TiledRight);
                        state.states.unset(State::TiledTop);
                        state.states.unset(State::TiledBottom);
                    }

                    let new_tiled_any = tiled_set.iter().any(|s| state.states.contains(*s));
                    state_changed = size_changed
                        || old_size != state.size
                        || old_activated != state.states.contains(State::Activated)
                        || old_fullscreen != state.states.contains(State::Fullscreen)
                        || old_tiled_any != new_tiled_any;
                });
                if size_changed {
                    resized.push(id);
                }
                if state_changed {
                    surface.send_configure();
                }
            }
        }

        if !resized.is_empty() {
            let count = resized.len();
            self.transaction_manager.create_transaction(resized);
            tracing::debug!("truss: resize transaction opened for {count} window(s)");
        }

        // Update layer shell geometries for bars/panels
        for output in &self.output_manager.outputs {
            let mut layer_map = layer_map_for_output(output);
            let _ = layer_map.arrange();
        }
    }

    /// Find which Wayland client surface is under the pointer across all layers (layer-shell, popups, windows).
    pub fn surface_under(
        &self,
        point: smithay::utils::Point<f64, smithay::utils::Logical>,
    ) -> Option<(
        smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
        smithay::utils::Point<f64, smithay::utils::Logical>,
    )> {
        use smithay::desktop::layer_map_for_output;
        use smithay::wayland::shell::wlr_layer::Layer;

        let active_ws = self.state.active_workspace_id;
        let px = point.x as i32;
        let py = point.y as i32;

        let check_window_under = |win_id: WindowId| -> Option<(
            smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
            smithay::utils::Point<f64, smithay::utils::Logical>,
        )> {
            if let Some(win) = self.state.windows.get(&win_id) {
                let r = &win.geometry;
                if px >= r.x && px < r.x + r.width as i32 && py >= r.y && py < r.y + r.height as i32
                {
                    if let Some(surface) = self.surfaces.get(&win_id) {
                        return Some((
                            surface.wl_surface().clone(),
                            smithay::utils::Point::from((r.x as f64, r.y as f64)),
                        ));
                    }
                }
            }
            None
        };

        let check_popups = |win_id: WindowId| -> Option<(
            smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
            smithay::utils::Point<f64, smithay::utils::Logical>,
        )> {
            if let (Some(surface), Some(win)) =
                (self.surfaces.get(&win_id), self.state.windows.get(&win_id))
            {
                let win_geom = (win.geometry.x, win.geometry.y);
                for (popup, popup_loc) in
                    smithay::desktop::PopupManager::popups_for_surface(surface.wl_surface())
                {
                    let popup_origin = smithay::utils::Point::from((
                        (win_geom.0 + popup_loc.x) as f64,
                        (win_geom.1 + popup_loc.y) as f64,
                    ));
                    let geom = popup.geometry();
                    let abs_rect = smithay::utils::Rectangle::new(
                        (
                            win_geom.0 + popup_loc.x + geom.loc.x,
                            win_geom.1 + popup_loc.y + geom.loc.y,
                        )
                            .into(),
                        geom.size,
                    );
                    if abs_rect.to_f64().contains(point) {
                        return Some((popup.wl_surface().clone(), popup_origin));
                    }
                }
            }
            None
        };

        // If a window is fullscreen, it takes precedence over even Overlay/Top layer surfaces (e.g. covers the bar)
        if let Some(ws) = self.state.workspaces.get(&active_ws) {
            for &win_id in ws.windows.iter().rev() {
                if let Some(win) = self.state.windows.get(&win_id) {
                    if win.fullscreen {
                        if let Some(hit) = check_popups(win_id) {
                            return Some(hit);
                        }
                        if let Some(hit) = check_window_under(win_id) {
                            return Some(hit);
                        }
                    }
                }
            }
        }

        // 1. Overlay & Top Layer Shell surfaces (e.g. Waybar, notifications, launchers)
        for output in &self.output_manager.outputs {
            let layer_map = layer_map_for_output(output);
            for layer in [Layer::Overlay, Layer::Top] {
                if let Some(surface) = layer_map.layer_under(layer, point) {
                    if let Some(geom) = layer_map.layer_geometry(surface) {
                        return Some((
                            surface.wl_surface().clone(),
                            smithay::utils::Point::from((geom.loc.x as f64, geom.loc.y as f64)),
                        ));
                    }
                }
            }
        }

        // 2. Floating windows and their popups on active workspace (always on top of tiled)
        if let Some(ws) = self.state.workspaces.get(&active_ws) {
            for &win_id in ws.windows.iter().rev() {
                if let Some(win) = self.state.windows.get(&win_id) {
                    if win.floating && !win.fullscreen {
                        if let Some(hit) = check_popups(win_id) {
                            return Some(hit);
                        }
                        if let Some(hit) = check_window_under(win_id) {
                            return Some(hit);
                        }
                    }
                }
            }
        }

        // 3. Tiled windows and their popups on active workspace
        if let Some(ws) = self.state.workspaces.get(&active_ws) {
            for &win_id in ws.windows.iter().rev() {
                if let Some(win) = self.state.windows.get(&win_id) {
                    if !win.floating && !win.fullscreen {
                        if let Some(hit) = check_popups(win_id) {
                            return Some(hit);
                        }
                        if let Some(hit) = check_window_under(win_id) {
                            return Some(hit);
                        }
                    }
                }
            }
        }

        // 4. Bottom & Background Layer Shell surfaces
        for output in &self.output_manager.outputs {
            let layer_map = layer_map_for_output(output);
            for layer in [Layer::Bottom, Layer::Background] {
                if let Some(surface) = layer_map.layer_under(layer, point) {
                    if let Some(geom) = layer_map.layer_geometry(surface) {
                        return Some((
                            surface.wl_surface().clone(),
                            smithay::utils::Point::from((geom.loc.x as f64, geom.loc.y as f64)),
                        ));
                    }
                }
            }
        }

        None
    }

    pub fn is_running(&self) -> bool {
        !self.shutdown.load(Ordering::SeqCst)
    }

    pub fn quit(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}
