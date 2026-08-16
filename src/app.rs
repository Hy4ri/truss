use std::collections::HashMap;

use smithay::{
    input::{Seat, SeatState},
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
    pub seat_state: SeatState<Self>,
    pub seat: Seat<Self>,
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
        let mut seat_state = SeatState::new();
        let seat = seat_state.new_wl_seat(&dh, "seat-0");

        let state = State::new();
        let mut dispatcher = Dispatcher::new();
        let ipc = IpcServer::new("truss.sock")?;
        ipc.setup_broadcaster(&mut dispatcher);

        Ok(Self {
            compositor_state,
            xdg_shell_state,
            shm_state,
            seat_state,
            seat,
            clients: Vec::new(),
            surfaces: HashMap::new(),
            state,
            dispatcher,
            ipc,
            shutdown: false,
        })
    }
}
