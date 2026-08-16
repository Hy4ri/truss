use smithay::{
    delegate_shm,
    wayland::shm::{ShmHandler, ShmState},
};

use crate::App;

impl ShmHandler for App {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_shm!(App);
