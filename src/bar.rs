use std::io::{Read, Write};
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
                // JSONL-safe read: a single read() may return a partial
                // response or several buffered lines. Read until we have at
                // least one full newline-terminated line.
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
                    }
                }
            }
            Err(_) => {
                print!("\r[truss-bar: waiting for compositor...]");
                let _ = std::io::stdout().flush();
            }
        }

        thread::sleep(Duration::from_millis(500));
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
    #[repr(C)]
    struct Tm {
        tm_sec: libc::c_int,
        tm_min: libc::c_int,
        tm_hour: libc::c_int,
        tm_mday: libc::c_int,
        tm_mon: libc::c_int,
        tm_year: libc::c_int,
        tm_wday: libc::c_int,
        tm_yday: libc::c_int,
        tm_isdst: libc::c_int,
        tm_gmtoff: libc::c_long,
        tm_zone: *const libc::c_char,
    }
    extern "C" {
        fn localtime_r(timep: *const libc::time_t, result: *mut Tm) -> *mut Tm;
    }
    let mut tm = Tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 0,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };
    let t = secs as libc::time_t;
    // SAFETY: `t` is a valid time_t and `tm` is a fully-owned Tm allocation.
    if unsafe {
        localtime_r(
            &t as *const libc::time_t,
            &mut tm as *mut Tm as *mut libc::tm,
        )
    }
    .is_null()
    {
        return format!("{secs} epoch");
    }
    let (hours, minutes, seconds) = (tm.tm_hour, tm.tm_min, tm.tm_sec);
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}
