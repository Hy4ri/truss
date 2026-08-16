# truss

A minimal, efficient, keyboard-first dynamic tiling Wayland compositor built in Rust on Smithay 0.7 (from scratch — no wlroots).

---

## Highlights

- **Pure State & Command Dispatcher**: Single source of truth in `src/state/` with an API-first command bus in `src/dispatch/`. Every action is atomic, validated, and broadcast.
- **Dynamic Master+Stack & Monocle Tiling**: Automatic layout calculations in `src/layout/` with configurable gaps and master ratio.
- **Universal JSONL IPC**: Control and query the compositor over `$XDG_RUNTIME_DIR/truss.sock` via newline-delimited JSON.
- **Lua Configuration & Scripting**: Embedded LuaJIT (`mlua`) with modular file inclusion via `truss.source("path.lua")`, custom settings, and `truss.on("event", fn)` hook callbacks.
- **Keyboard-First & Interactive Pointer**: Built-in keybinding engine (`Super+Return` terminal launcher, workspace switching) with interactive Mod-drag move & resize.
- **Multi-Output & Desktop Compositing**: Smithay `Space` compositing pipeline with multi-monitor arrangement, fractional scale, and refresh rate tracking.

---

## Directory Architecture

```
truss/
├── flake.nix               # Nix Flake providing Rust toolchain & Wayland/Smithay C dependencies
├── .envrc                  # direnv configuration (`use flake`)
├── Cargo.toml              # Dependencies: smithay 0.7, mlua (luajit), serde, calloop
└── src/
    ├── main.rs             # Bootloader: Wayland socket setup, Winit host loop & event pump
    ├── lib.rs              # Public library exports
    ├── app.rs              # Compositor context: Seats, Protocols, Managers, Dispatcher
    ├── state/              # Pure state model (Window, Workspace, State)
    ├── dispatch/           # Command bus (Command, Event, Dispatcher)
    ├── layout/             # Pure arrange contract (MasterLayout, MonocleLayout, Registry)
    ├── input/              # Input subsystem (Keybindings, Modifiers, PointerState)
    ├── protocols/          # Protocol delegates (XDG Shell, SHM, Seat, Data Device)
    ├── backend/            # OutputManager, Space RenderManager, display geometries
    ├── config/             # LuaJIT runtime (LuaConfig, truss global table, on() hooks)
    └── ipc/                # JSONL UNIX socket IPC server & broadcaster
```

---

## Building & Running

### Requirements
- Rust 1.75+ (or stable)
- Wayland dev libraries: `libwayland`, `libxkbcommon`, `libinput`, `libudev`, `libseat`, `libgbm`, `pixman`, `mesa/egl`

### Using Nix (Recommended)
```bash
# Enter dev shell with all C headers and toolchains configured
nix develop

# Or with direnv
direnv allow

# Run test suite
cargo test --all-features

# Build optimized release binary
cargo build --release
```

### Launching the Compositor
```bash
./target/release/truss
```
*Truss will create a Wayland display socket at `WAYLAND_DISPLAY=truss-0` and open an interactive graphical window.*

---

## Default Keybindings

| Key Combo | Action | Description |
| :--- | :--- | :--- |
| `Super + Return` | `Spawn("foot")` | Launch terminal emulator inside `truss` |
| `Super + Shift + Q` | `CompositorQuit` | Gracefully shut down compositor |
| `Super + J` | `WindowFocusDir(Next)` | Focus next window in layout |
| `Super + K` | `WindowFocusDir(Prev)` | Focus previous window in layout |
| `Super + Space` | `WindowSwapMaster` | Swap currently focused window with master |
| `Super + 1..9` | `WorkspaceSwitch(id)` | Switch active workspace (1 through 9) |
| `Super + Shift + 1..9` | `WindowMoveToWorkspace`| Move focused window to workspace (1 through 9) |
| `Super + Left Click Drag` | `PointerDragMove` | Interactively drag and move window (floats window) |
| `Super + Right Click Drag`| `PointerDragResize`| Interactively drag and resize window (floats window)|
| `Left Click` | `WindowFocus` | Focus window under pointer |

---

## Lua Configuration

Place your configuration in `~/.config/truss/config.lua` or `$XDG_CONFIG_HOME/truss/config.lua`:

```lua
-- ~/.config/truss/config.lua

-- Layout preferences
gap = 12
master_ratio = 0.55

-- Modular inclusion
-- truss.source("~/.config/truss/keybinds.lua")

-- Event listeners
truss.on("window.focused", function(ev)
    -- print("Window focused:", ev.id)
end)

truss.on("workspace.switched", function(ev)
    -- print("Switched to workspace:", ev.id)
end)
```

---

## JSONL IPC Socket

Connect to `$XDG_RUNTIME_DIR/truss.sock` via UNIX domain socket:

### 1. Send Commands
```bash
# Query complete state tree
echo '{"command": "state.get"}' | nc -U $XDG_RUNTIME_DIR/truss.sock

# Switch workspace
echo '{"command": "workspace.switch", "params": {"id": 2}}' | nc -U $XDG_RUNTIME_DIR/truss.sock

# Adjust layout gap
echo '{"command": "layout.set_gap", "params": {"gap": 20}}' | nc -U $XDG_RUNTIME_DIR/truss.sock

# Focus window
echo '{"command": "window.focus", "params": {"id": 1}}' | nc -U $XDG_RUNTIME_DIR/truss.sock
```

### 2. Subscribe to Event Stream
```bash
# Subscribe to all compositor events
echo '{"subscribe": "all"}' | nc -U $XDG_RUNTIME_DIR/truss.sock
```
*Outputs JSON events such as `window.created`, `window.focused`, `workspace.switched`, `layout.changed` in real-time.*

---

## Testing & Quality Assurance

Truss enforces zero-warning builds and 100% test pass rates across all subsystems:

```bash
# Check formatting
cargo fmt --all -- --check

# Check clippy warnings
cargo clippy --all-targets --all-features -- -D warnings

# Run all 24 unit & integration test suites
cargo test --all-features
```

---

## License

MIT / Apache 2.0
