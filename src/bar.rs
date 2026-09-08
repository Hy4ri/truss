use std::io::{BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;
use std::time::Duration;

use crate::ipc::protocol::{IpcRequest, IpcResponse};
use crate::state::State;

/// Run the interactive CLI status bar reading compositor state via IPC
pub fn run_status_bar(socket_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let socket_path = Path::new(&runtime_dir).join(socket_name);

    println!(
        "truss-bar: connecting to compositor socket at {:?}",
        socket_path
    );

    let mut last_state: Option<State> = None;

    loop {
        match UnixStream::connect(&socket_path) {
            Ok(mut stream) => {
                let req = IpcRequest {
                    id: Some(1),
                    command: crate::dispatch::Command::StateGet,
                };
                let req_json = serde_json::to_string(&req)?;
                stream.write_all(format!("{req_json}\n").as_bytes())?;

                let mut acc: Vec<u8> = Vec::new();
                let mut chunk = [0u8; 1024];
                loop {
                    match stream.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            acc.extend_from_slice(&chunk[..n]);
                            if acc.contains(&b'\n') {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let line = acc
                    .split(|&b| b == b'\n')
                    .find(|l| !l.is_empty())
                    .unwrap_or(&[]);
                if let Ok(resp) = serde_json::from_slice::<IpcResponse>(line) {
                    if let Some(crate::dispatch::DispatchResult::State(state)) = resp.result {
                        render_bar_line(&state);
                        last_state = Some(state);
                    }
                }

                // Keep stream and listen for streaming event updates instead of polling aggressively
                let reader = std::io::BufReader::new(stream);
                for l in reader.lines() {
                    let Ok(line) = l else { break };
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(event_msg) =
                        serde_json::from_str::<crate::ipc::protocol::IpcEventMessage>(trimmed)
                    {
                        match event_msg.event {
                            crate::dispatch::Event::WorkspaceSwitched { id } => {
                                if let Some(ref mut st) = last_state {
                                    st.active_workspace_id = id;
                                    render_bar_line(st);
                                }
                            }
                            crate::dispatch::Event::WindowFocused { id } => {
                                if let Some(ref mut st) = last_state {
                                    st.active_workspace_mut().focused_window = Some(id);
                                    render_bar_line(st);
                                }
                            }
                            _ => {
                                // Full re-query on structural layout changes
                                break;
                            }
                        }
                    }
                }
            }
            Err(_) => {
                print!("\r[truss-bar: waiting for compositor...]");
                let _ = std::io::stdout().flush();
                thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

fn render_bar_line(state: &State) {
    let active_ws = state.active_workspace_id;
    let mut ws_str = String::new();

    for id in state.workspaces.keys() {
        if *id == active_ws {
            ws_str.push_str(&format!("[{id}] "));
        } else {
            ws_str.push_str(&format!(" {id}  "));
        }
    }

    let active_title = state
        .active_workspace()
        .focused_window
        .and_then(|id| state.windows.get(&id))
        .and_then(|w| w.title.as_deref().or(w.app_id.as_deref()))
        .unwrap_or("~");

    let now = chrono_or_fallback_time();
    let total_wins = state.windows.len();
    let layout_name = &state.active_workspace().layout;

    print!(
        "\r\x1b[2K\x1b[1;36mtruss\x1b[0m | \x1b[1;32m{}\x1b[0m | \x1b[1;33mlayout:\x1b[0m {} | \x1b[1;34mwin:\x1b[0m {} | \x1b[1;37m{}\x1b[0m | \x1b[1;35m{}\x1b[0m",
        ws_str.trim_end(),
        layout_name,
        total_wins,
        active_title,
        now
    );
    let _ = std::io::stdout().flush();
}

fn chrono_or_fallback_time() -> String {
    let now = std::time::SystemTime::now();
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Local wall-clock time via libc (respects TZ), no chrono dependency.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let t = secs as libc::time_t;
    // SAFETY: `t` points to a valid time_t, `tm` is a fully-owned allocation.
    if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
        return format!("{secs} epoch");
    }
    let (hours, minutes, seconds) = (tm.tm_hour, tm.tm_min, tm.tm_sec);
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}
