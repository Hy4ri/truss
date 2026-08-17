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
use tracing::info;

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
    pub seat_state: SeatState<Self>,
    pub seat: Seat<Self>,
    pub keyboard: Option<KeyboardHandle<Self>>,
    pub pointer: Option<PointerHandle<Self>>,
    pub pointer_state: PointerState,
    pub cursor_status: CursorImageStatus,
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

        if let Some(config_path) = LuaConfig::find_default_config_path() {
            info!("Loading configuration from {}", config_path.display());
            if let Ok(()) = lua_config.load_file(&config_path) {
                lua_config.apply_to_dispatcher(&mut dispatcher);
                let mut rules_mgr = WindowRuleManager::new();
                lua_config.apply_rules_to_manager(&mut rules_mgr);
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
            seat_state,
            seat,
            keyboard: Some(keyboard),
            pointer: Some(pointer),
            pointer_state: PointerState::new(),
            cursor_status: CursorImageStatus::default_named(),
            keybindings: Keybindings::new_default(),
            window_rules: WindowRuleManager::new(),
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

        let focused = self.state.active_workspace().focused_window;
        for (id, surface) in &self.surfaces {
            let is_active = Some(*id) == focused;
            surface.with_pending_state(|state| {
                if is_active {
                    state.states.set(
                        smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Activated,
                    );
                } else {
                    state.states.unset(
                        smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Activated,
                    );
                }
            });
            surface.send_configure();
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

        // Update layer shell geometries for bars/panels
        for output in &self.output_manager.outputs {
            let mut layer_map = layer_map_for_output(output);
            let _ = layer_map.arrange();
        }
    }

    pub fn is_running(&self) -> bool {
        !self.shutdown.load(Ordering::SeqCst)
    }

    pub fn quit(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }
}
