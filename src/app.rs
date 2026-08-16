use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use smithay::{
    input::{
        keyboard::{KeyboardHandle, XkbConfig},
        pointer::PointerHandle,
        Seat, SeatState,
    },
    reexports::wayland_server::Display,
    wayland::{
        compositor::CompositorState,
        data_device::DataDeviceState,
        shell::xdg::{ToplevelSurface, XdgShellState},
        shm::ShmState,
    },
};
use tracing::info;
use wayland_server::Client;

use crate::{
    backend::{OutputManager, RenderManager},
    config::LuaConfig,
    dispatch::Dispatcher,
    input::{Keybindings, PointerState},
    ipc::IpcServer,
    state::{State, WindowId},
};

pub struct App {
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub data_device_state: DataDeviceState,
    pub seat_state: SeatState<Self>,
    pub seat: Seat<Self>,
    pub keyboard: Option<KeyboardHandle<Self>>,
    pub pointer: Option<PointerHandle<Self>>,
    pub pointer_state: PointerState,
    pub keybindings: Keybindings,
    pub output_manager: OutputManager,
    pub render_manager: RenderManager,
    pub lua_config: LuaConfig,
    pub clients: Vec<Client>,
    pub surfaces: HashMap<WindowId, ToplevelSurface>,
    pub state: State,
    pub dispatcher: Dispatcher,
    pub ipc: IpcServer,
    pub shutdown: Arc<AtomicBool>,
}

impl App {
    pub fn new(display: &mut Display<Self>) -> Result<Self, std::io::Error> {
        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(&dh, "seat-0");

        // Initialize keyboard & pointer on seat
        let keyboard = seat
            .add_keyboard(XkbConfig::default(), 200, 25)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let pointer = seat.add_pointer();

        let mut output_manager = OutputManager::new();
        output_manager.create_default_output("HEADLESS-1", (1920, 1080).into());

        let render_manager = RenderManager::new();

        let lua_config = LuaConfig::new()
            .map_err(|e| std::io::Error::other(format!("Lua initialization failed: {e}")))?;

        let state = State::new();
        let mut dispatcher = Dispatcher::new();

        if let Some(config_path) = LuaConfig::find_default_config_path() {
            info!("Loading configuration from {}", config_path.display());
            if let Ok(()) = lua_config.load_file(&config_path) {
                lua_config.apply_to_dispatcher(&mut dispatcher);
            }
        }

        let ipc = IpcServer::new("truss.sock")?;
        ipc.setup_broadcaster(&mut dispatcher);

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = shutdown.clone();

        dispatcher.subscribe(move |event| {
            if let crate::dispatch::Event::CompositorQuitting = event {
                shutdown_clone.store(true, Ordering::SeqCst);
            }
        });

        Ok(Self {
            compositor_state,
            xdg_shell_state,
            shm_state,
            data_device_state,
            seat_state,
            seat,
            keyboard: Some(keyboard),
            pointer: Some(pointer),
            pointer_state: PointerState::new(),
            keybindings: Keybindings::new_default(),
            output_manager,
            render_manager,
            lua_config,
            clients: Vec::new(),
            surfaces: HashMap::new(),
            state,
            dispatcher,
            ipc,
            shutdown,
        })
    }

    pub fn is_running(&self) -> bool {
        !self.shutdown.load(Ordering::SeqCst)
    }

    /// Request clean shutdown
    pub fn quit(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Refresh layout calculations for active workspace and synchronize with Space elements.
    pub fn refresh_layout_and_space(&mut self) {
        let usable_area = self.output_manager.primary_usable_area();
        let active_ws_id = self.state.active_workspace_id;
        self.dispatcher
            .recalculate_workspace_layout(&mut self.state, active_ws_id, usable_area);
        self.render_manager
            .sync_windows(&self.state, &self.surfaces);
    }
}
