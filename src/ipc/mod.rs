pub mod protocol;

use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tracing::{error, info, warn};

pub use protocol::{IpcEventMessage, IpcRequest, IpcResponse};

use crate::{
    dispatch::{Dispatcher, Event},
    state::State,
};

/// IPC Server managing the `$XDG_RUNTIME_DIR/truss.sock` UNIX socket.
pub struct IpcServer {
    socket_path: PathBuf,
    listener: Option<UnixListener>,
    clients: Arc<Mutex<Vec<UnixStream>>>,
}

impl IpcServer {
    pub fn new(socket_name: &str) -> Result<Self, std::io::Error> {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
        let socket_path = Path::new(&runtime_dir).join(socket_name);

        if socket_path.exists() {
            let _ = fs::remove_file(&socket_path);
        }

        let listener = UnixListener::bind(&socket_path)?;
        listener.set_nonblocking(true)?;

        info!("truss: IPC socket listening at {:?}", socket_path);

        Ok(Self {
            socket_path,
            listener: Some(listener),
            clients: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn setup_broadcaster(&self, dispatcher: &mut Dispatcher) {
        let clients = self.clients.clone();
        dispatcher.subscribe(move |event: &Event| {
            let msg = match serde_json::to_string(&IpcEventMessage {
                event: event.clone(),
            }) {
                Ok(json) => format!("{json}\n"),
                Err(e) => {
                    error!("Failed to serialize event: {e}");
                    return;
                }
            };

            let mut guard = match clients.lock() {
                Ok(g) => g,
                Err(_) => return,
            };

            guard.retain_mut(|client| {
                if let Err(e) = client.write_all(msg.as_bytes()) {
                    warn!("Client write error, dropping client: {e}");
                    false
                } else {
                    true
                }
            });
        });
    }

    /// Non-blocking poll for new connections and incoming requests.
    pub fn poll_and_dispatch(&mut self, state: &mut State, dispatcher: &mut Dispatcher) {
        // Accept new connections
        if let Some(ref listener) = self.listener {
            while let Ok((stream, _)) = listener.accept() {
                if let Ok(()) = stream.set_nonblocking(true) {
                    if let Ok(mut clients) = self.clients.lock() {
                        clients.push(stream);
                    }
                }
            }
        }

        // Process incoming lines from clients
        let mut clients = match self.clients.lock() {
            Ok(c) => c,
            Err(_) => return,
        };

        clients.retain_mut(|client| {
            let mut reader = BufReader::new(client.try_clone().unwrap());
            let mut line = String::new();

            match reader.read_line(&mut line) {
                Ok(0) => false, // EOF / Client disconnected
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        return true;
                    }

                    let response = match serde_json::from_str::<IpcRequest>(trimmed) {
                        Ok(req) => match dispatcher.dispatch(state, req.command) {
                            Ok(res) => IpcResponse::success(req.id, res),
                            Err(err) => IpcResponse::error(req.id, err),
                        },
                        Err(parse_err) => IpcResponse {
                            id: None,
                            ok: false,
                            result: None,
                            error: Some(format!("Invalid JSON request: {parse_err}")),
                        },
                    };

                    if let Ok(resp_json) = serde_json::to_string(&response) {
                        let _ = client.write_all(format!("{resp_json}\n").as_bytes());
                    }
                    true
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => true,
                Err(_) => false,
            }
        });
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        if self.socket_path.exists() {
            let _ = fs::remove_file(&self.socket_path);
        }
    }
}
