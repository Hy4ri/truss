use std::{collections::HashMap, sync::Arc};

use smithay::{
    reexports::wayland_server::Display,
    wayland::{
        compositor::CompositorState,
        shell::xdg::{ToplevelSurface, XdgShellState},
        shm::ShmState,
    },
};
use wayland_server::Client;

use crate::{
    dispatch::Dispatcher,
    ipc::IpcServer,
    state::{State, WindowId},
};

pub struct App {
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub clients: Vec<Client>,
    pub surfaces: HashMap<WindowId, ToplevelSurface>,
    pub state: State,
    pub dispatcher: Dispatcher,
    pub ipc: IpcServer,
    pub shutdown: bool,
}

impl App {
    pub fn new(display: &mut Display<Self>) -> Result<Self, std::io::Error> {
        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);

        let state = State::new();
        let mut dispatcher = Dispatcher::new();
        let ipc = IpcServer::new("truss.sock")?;
        ipc.setup_broadcaster(&mut dispatcher);

        Ok(Self {
            compositor_state,
            xdg_shell_state,
            shm_state,
            clients: Vec::new(),
            surfaces: HashMap::new(),
            state,
            dispatcher,
            ipc,
            shutdown: false,
        })
    }
}
