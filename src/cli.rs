use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use crate::config::{LuaConfig, DEFAULT_CONFIG};
use crate::dispatch::{Command, Direction};
use crate::ipc::protocol::{IpcRequest, IpcResponse};
use crate::state::WindowId;

#[derive(Debug, PartialEq, Eq)]
pub enum Subcommand {
    Msg(Vec<String>),
    Bar,
    InitConfig,
    Version,
    Help,
}

#[derive(Debug)]
pub struct CliArgs {
    pub config_path: Option<PathBuf>,
    pub socket_name: String,
    pub backend: Option<String>,
    pub subcommand: Option<Subcommand>,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            config_path: None,
            socket_name: "truss-0".into(),
            backend: None,
            subcommand: None,
        }
    }
}

impl CliArgs {
    pub fn parse() -> Self {
        Self::parse_from(std::env::args().skip(1))
    }

    /// Pure core of [`Self::parse`]: parses arguments from an explicit
    /// iterator instead of process args, so the logic is testable.
    pub fn parse_from(mut args: impl Iterator<Item = String>) -> Self {
        let mut cli = Self::default();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-c" | "--config" => {
                    if let Some(path) = args.next() {
                        cli.config_path = Some(PathBuf::from(path));
                    }
                }
                "-s" | "--socket" => {
                    if let Some(sock) = args.next() {
                        cli.socket_name = sock;
                    }
                }
                "-b" | "--backend" => {
                    if let Some(b) = args.next() {
                        cli.backend = Some(b.to_lowercase());
                    }
                }
                "-V" | "--version" => {
                    cli.subcommand = Some(Subcommand::Version);
                    return cli;
                }
                "-h" | "--help" => {
                    cli.subcommand = Some(Subcommand::Help);
                    return cli;
                }
                "bar" => {
                    cli.subcommand = Some(Subcommand::Bar);
                    return cli;
                }
                "init-config" => {
                    cli.subcommand = Some(Subcommand::InitConfig);
                    return cli;
                }
                "msg" => {
                    let rest: Vec<String> = args.collect();
                    cli.subcommand = Some(Subcommand::Msg(rest));
                    return cli;
                }
                unknown => {
                    eprintln!("truss: unknown argument: {unknown}");
                    cli.subcommand = Some(Subcommand::Help);
                    return cli;
                }
            }
        }

        cli
    }

    pub fn print_help() {
        println!(
            r#"truss {} — dynamic tiling Wayland compositor

USAGE:
    truss [OPTIONS] [SUBCOMMAND]

OPTIONS:
    -c, --config <PATH>       Path to Lua configuration file
    -s, --socket <NAME>       Wayland socket name (default: truss-0)
    -b, --backend <BACKEND>   Force backend: winit, tty, or headless
    -V, --version             Print version information
    -h, --help                Print this help message

SUBCOMMANDS:
    msg <COMMAND> [ARGS...]   Send IPC message to running compositor
    bar                       Run live status bar companion
    init-config               Write the default configuration to ~/.config/truss/config.lua

IPC COMMANDS:
    truss msg state-get                  Fetch complete compositor state
    truss msg workspace-switch <ID>      Switch to workspace (1-9)
    truss msg window-focus-dir <DIR>     Focus next/prev window (next, prev)
    truss msg swap-master                Swap focused window with master
    truss msg close-window [ID]          Close target/focused window
    truss msg toggle-floating [ID]       Toggle floating mode
    truss msg toggle-fullscreen [ID]     Toggle fullscreen mode
    truss msg layout-set <LAYOUT>        Set active workspace layout (master, monocle)
    truss msg set-gap <PIXELS>           Set window inner/outer gap
    truss msg set-ratio <FLOAT>          Set master area ratio (0.1 - 0.9)
    truss msg spawn <CMD>                Spawn process inside compositor environment
    truss msg quit                       Cleanly shutdown compositor
"#,
            env!("CARGO_PKG_VERSION")
        );
    }

    pub fn print_version() {
        println!("truss {}", env!("CARGO_PKG_VERSION"));
    }
}

/// Send IPC command to running truss compositor instance over UNIX socket
pub fn handle_msg_command(
    args: &[String],
    socket_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.is_empty() {
        eprintln!("truss msg: missing command. Run `truss --help` for available commands.");
        std::process::exit(1);
    }

    let command_name = args[0].as_str();
    let command = match command_name {
        "state-get" => Command::StateGet,
        "workspace-switch" => {
            let id: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
            Command::WorkspaceSwitch { id }
        }
        "window-focus-dir" => {
            let dir_str = args.get(1).map(|s| s.as_str()).unwrap_or("next");
            let direction = match dir_str {
                "prev" => Direction::Prev,
                _ => Direction::Next,
            };
            Command::WindowFocusDir { direction }
        }
        "swap-master" => Command::WindowSwapMaster,
        "close-window" => {
            let id = args
                .get(1)
                .and_then(|s| s.parse::<u64>().ok())
                .map(WindowId);
            Command::WindowClose { id }
        }
        "toggle-floating" => {
            let id = args
                .get(1)
                .and_then(|s| s.parse::<u64>().ok())
                .map(WindowId);
            Command::WindowToggleFloating { id }
        }
        "toggle-fullscreen" => {
            let id = args
                .get(1)
                .and_then(|s| s.parse::<u64>().ok())
                .map(WindowId);
            Command::WindowToggleFullscreen { id }
        }
        "layout-set" => {
            let layout = args.get(1).cloned().unwrap_or_else(|| "master".into());
            Command::LayoutSet { layout }
        }
        "set-gap" => {
            let gap: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(8);
            Command::LayoutSetGap { gap }
        }
        "set-ratio" => {
            let ratio: f32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.55);
            Command::LayoutSetRatio { ratio }
        }
        "spawn" => {
            let cmd = args[1..].join(" ");
            Command::Spawn { command: cmd }
        }
        "quit" => Command::CompositorQuit,
        unknown => {
            eprintln!("truss msg: unknown command '{unknown}'.");
            std::process::exit(1);
        }
    };

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let socket_path = Path::new(&runtime_dir).join(format!("{socket_name}.sock"));

    let mut stream = UnixStream::connect(&socket_path).map_err(|e| {
        format!(
            "Failed to connect to truss socket at {:?}: {e}\nIs truss running?",
            socket_path
        )
    })?;

    let req = IpcRequest {
        id: Some(1),
        command,
    };
    let req_json = serde_json::to_string(&req)?;
    stream.write_all(format!("{req_json}\n").as_bytes())?;

    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    if n > 0 {
        let resp: IpcResponse = serde_json::from_slice(&buf[..n])?;
        if resp.ok {
            if let Some(res) = resp.result {
                println!("{}", serde_json::to_string_pretty(&res)?);
            } else {
                println!("OK");
            }
        } else {
            eprintln!(
                "Error: {}",
                resp.error.unwrap_or_else(|| "Unknown error".into())
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Write the embedded default configuration to `path`, creating parent
/// directories as needed. Refuses to overwrite an existing file.
pub fn write_default_config(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "{} already exists, not overwriting",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create directory {}: {e}", parent.display()))?;
    }
    std::fs::write(path, DEFAULT_CONFIG)
        .map_err(|e| format!("failed to write {}: {e}", path.display()))
}

/// Write the embedded default configuration to the user's config directory.
/// Refuses to overwrite an existing file.
pub fn handle_init_config_command() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = LuaConfig::default_user_config_path() else {
        eprintln!(
            "truss init-config: cannot determine user config directory \
             (neither XDG_CONFIG_HOME nor HOME is set)"
        );
        std::process::exit(1);
    };
    if let Err(e) = write_default_config(&path) {
        eprintln!("truss init-config: {e}");
        std::process::exit(1);
    }
    println!("truss: default configuration written to {}", path.display());
    Ok(())
}
