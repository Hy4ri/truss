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
use tracing::{info, warn};

use crate::{
    backend::{OutputManager, RenderManager},
    config::LuaConfig,
    dispatch::{Dispatcher, Event},
    input::{Keybindings, PointerState},
    ipc::IpcServer,
    state::{State, WindowId, WindowRuleManager},
    sync::TransactionManager,
};

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
}

impl App {
    pub fn new(display: &mut Display<Self>) -> Result<Self, std::io::Error> {
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
        let mut window_rules = WindowRuleManager::new();

        if let Some(config_path) = LuaConfig::find_default_config_path() {
            info!("Loading configuration from {}", config_path.display());
            if let Ok(()) = lua_config.load_file(&config_path) {
                lua_config.apply_to_dispatcher(&mut dispatcher);
                lua_config.apply_rules_to_manager(&mut window_rules);
            }
        }

        let ipc = IpcServer::new("truss.sock")?;
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
            keybindings: Keybindings::new_default(),
            window_rules,
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

    /// Deliver dispatcher events to Lua hooks without blocking the compositor.
    pub fn process_pending_events(&self) {
        for event in self.event_rx.try_iter() {
            self.lua_config.handle_event(&event);
        }
    }

    /// Refresh and recalculate layouts for active workspaces across outputs.
    pub fn refresh_layout_and_space(&mut self) {
        // Cleanup dead popup trees periodically
        self.popups.cleanup();

        // Prune expired sync transactions
        self.transaction_manager.prune_expired();

        let area = self.output_manager.primary_usable_area();
        let active_ws = self.state.active_workspace_id;
        self.dispatcher
            .recalculate_workspace_layout(&mut self.state, active_ws, area);
        self.render_manager
            .sync_windows(&self.state, &self.surfaces);

        // Update toplevel window surface states and configure sizes
        let focused = self.state.active_workspace().focused_window;
        let bounds_size = smithay::utils::Size::from((area.width as i32, area.height as i32));

        for (&id, surface) in &self.surfaces {
            if let Some(win) = self.state.windows.get(&id) {
                let is_on_active_ws = win.workspace_id == active_ws;
                let is_active = is_on_active_ws && Some(id) == focused;

                surface.with_pending_state(|state| {
                    use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State;

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

                    if is_on_active_ws && !win.floating {
                        state.states.set(State::TiledLeft);
                        state.states.set(State::TiledRight);
                        state.states.set(State::TiledTop);
                        state.states.set(State::TiledBottom);
                        state.bounds = Some(bounds_size);
                        state.size = Some(
                            (win.geometry.width as i32, win.geometry.height as i32).into(),
                        );
                    } else {
                        state.states.unset(State::TiledLeft);
                        state.states.unset(State::TiledRight);
                        state.states.unset(State::TiledTop);
                        state.states.unset(State::TiledBottom);
                    }
                });
                surface.send_configure();
            }
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

        // 1. Overlay & Top Layer Shell surfaces
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

        // 2. Popups associated with toplevel windows on active workspace
        let active_ws = self.state.active_workspace_id;
        for surface in self.xdg_shell_state.toplevel_surfaces() {
            let win_entry = self
                .surfaces
                .iter()
                .find(|(_, s)| s.wl_surface() == surface.wl_surface())
                .and_then(|(id, _)| self.state.windows.get(id));

            if let Some(win) = win_entry {
                if win.workspace_id != active_ws {
                    continue;
                }
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
        }

        // 3. Toplevel Windows on active workspace
        let px = point.x as i32;
        let py = point.y as i32;
        if let Some(ws) = self.state.workspaces.get(&active_ws) {
            for &win_id in ws.windows.iter().rev() {
                if let Some(win) = self.state.windows.get(&win_id) {
                    let r = &win.geometry;
                    if px >= r.x
                        && px < r.x + r.width as i32
                        && py >= r.y
                        && py < r.y + r.height as i32
                    {
                        if let Some(surface) = self.surfaces.get(&win_id) {
                            return Some((
                                surface.wl_surface().clone(),
                                smithay::utils::Point::from((r.x as f64, r.y as f64)),
                            ));
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
