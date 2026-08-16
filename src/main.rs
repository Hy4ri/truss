use std::{
    sync::Arc,
    time::Duration,
};

use smithay::{
    delegate_compositor,
    reexports::{
        calloop::{
            generic::Generic,
            timer::{Timer, TimeoutAction},
            EventLoop, Interest, Mode, PostAction,
        },
        wayland_server::Display,
    },
    wayland::compositor::{CompositorClientState, CompositorHandler, CompositorState},
};
use tracing::{info, warn};
use wayland_server::{
    backend::{ClientData, ClientId, DisconnectReason},
    protocol::wl_surface::WlSurface,
    Client, ListeningSocket,
};

struct App {
    compositor_state: CompositorState,
    clients: Vec<Client>,
    shutdown: bool,
}

impl AsMut<CompositorState> for App {
    fn as_mut(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }
}

impl CompositorHandler for App {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        info!("surface committed: {:?}", surface);
    }
}

delegate_compositor!(App);

#[derive(Default)]
struct ClientState {
    compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {
        info!("client connected");
    }

    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {
        info!("client disconnected");
    }
}

const SOCKET_NAME: &str = "truss-0";
const ALIVE_SECONDS: u64 = 8;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let mut display: Display<App> = Display::new()?;
    let dh = display.handle();
    let mut listener_dh = dh.clone();

    let compositor_state = CompositorState::new::<App>(&dh);

    let mut state = App {
        compositor_state,
        clients: Vec::new(),
        shutdown: false,
    };

    let mut event_loop = EventLoop::<App>::try_new()?;
    let loop_handle = event_loop.handle();

    let listener = ListeningSocket::bind(SOCKET_NAME)?;
    info!("truss: wayland socket live at WAYLAND_DISPLAY={SOCKET_NAME}");

    // Accept new wayland clients.
    loop_handle.insert_source(
        Generic::new(listener, Interest::READ, Mode::Level),
        move |_, listener, state: &mut App| {
            while let Some(stream) = listener.accept()? {
                let client = listener_dh
                    .insert_client(stream, Arc::new(ClientState::default()))?;
                state.clients.push(client);
            }
            Ok(PostAction::Continue)
        },
    )?;

    // Clean shutdown after ALIVE_SECONDS of verified liveness.
    let signal = event_loop.get_signal();
    loop_handle.insert_source(
        Timer::from_duration(Duration::from_secs(ALIVE_SECONDS)),
        move |_, _, state: &mut App| {
            info!(
                "truss: event loop alive for {ALIVE_SECONDS}s with {} client(s) — clean shutdown",
                state.clients.len()
            );
            state.shutdown = true;
            signal.stop();
            TimeoutAction::Drop
        },
    )?;

    while !state.shutdown {
        event_loop.dispatch(Duration::from_millis(10), &mut state)?;
        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;
    }

    warn!("truss: shutting down");
    Ok(())
}
