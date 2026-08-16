use std::{sync::Arc, time::Duration};

use smithay::reexports::{
    calloop::{
        generic::Generic,
        timer::{TimeoutAction, Timer},
        EventLoop, Interest, Mode, PostAction,
    },
    wayland_server::Display,
};
use tracing::{info, warn};
use wayland_server::ListeningSocket;

use truss::{protocols::compositor::ClientState, App};

const WAYLAND_SOCKET: &str = "truss-0";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let mut display: Display<App> = Display::new()?;
    let mut app = App::new(&mut display)?;

    let dh = display.handle();
    let mut listener_dh = dh.clone();

    let mut event_loop = EventLoop::<App>::try_new()?;
    let loop_handle = event_loop.handle();

    let listener = ListeningSocket::bind(WAYLAND_SOCKET)?;
    info!("truss: wayland compositor running live at WAYLAND_DISPLAY={WAYLAND_SOCKET}");
    info!("truss: ready for clients! Launch apps with `WAYLAND_DISPLAY={WAYLAND_SOCKET} <app>`");

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

    // Periodic IPC poll & layout refresh
    loop_handle.insert_source(
        Timer::from_duration(Duration::from_millis(16)),
        move |_, _, state: &mut App| {
            state
                .ipc
                .poll_and_dispatch(&mut state.state, &mut state.dispatcher);
            state.refresh_layout_and_space();
            TimeoutAction::ToDuration(Duration::from_millis(16))
        },
    )?;

    while app.is_running() {
        event_loop.dispatch(Duration::from_millis(10), &mut app)?;
        display.dispatch_clients(&mut app)?;
        display.flush_clients()?;
    }

    warn!("truss: shutting down cleanly");
    Ok(())
}
