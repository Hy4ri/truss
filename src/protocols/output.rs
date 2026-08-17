use smithay::{delegate_output, wayland::output::OutputHandler};

use crate::App;

impl OutputHandler for App {}

delegate_output!(App);
