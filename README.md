# truss

A minimal, efficient, keyboard-first dynamic tiling Wayland compositor built in Rust on Smithay 0.7 (from scratch — no wlroots).

---

## Highlights

- **Pure State & Command Dispatcher**: Single source of truth in `src/state/` with an API-first command bus in `src/dispatch/`. Every action is atomic, validated, and broadcast.
- **Dynamic Master+Stack & Monocle Tiling**: Automatic layout calculations in `src/layout/` with configurable gaps and master ratio.
- **Declarative Window Rules**: Match by `app_id` and `title` to automatically float windows (e.g. `pavucontrol`, `mpv`), pin to workspaces, or open fullscreen.
- **Universal JSONL IPC & CLI**: Send commands via `truss msg <CMD>` or query full state over `$XDG_RUNTIME_DIR/truss.sock`.
- **Live Status Bar Companion**: Built-in `truss bar` companion status line displaying workspaces, active window, layout, and clock.
- **Lua Configuration & Autostart**: Embedded LuaJIT (`mlua`) with modular includes (`truss.source`), `truss.spawn_at_startup`, and reactive `truss.on("event", fn)` hooks.
- **Bare-Metal TTY & Nested Support**: Runs inside existing Wayland/X11 desktops via Winit or directly from Linux TTY console using `libseat`, `libinput`, and DRM/KMS.
- **Multi-GPU & Nvidia Ready**: Configured with `egl-wayland`, `GBM_BACKENDS_PATH`, and Mesa runtime libraries.

---

## Directory Architecture

```
truss/
├── flake.nix               # Nix Flake providing Rust toolchain & Wayland/Smithay C dependencies
├── .envrc                  # direnv configuration (`use flake`)
├── Cargo.toml              # Dependencies: smithay 0.7, mlua (luajit), serde, calloop
├── resources/
│   └── config.default.lua  # Reference starter configuration template
└── src/
    ├── main.rs             # Bootloader, launch flags, backend selection & main loop
    ├── lib.rs              # Public library exports
    ├── app.rs              # Compositor context: Seats, Protocols, Managers, Dispatcher
    ├── cli.rs              # CLI flag parser & `truss msg` IPC command sender
    ├── bar.rs              # Companion status bar client (`truss bar`)
    ├── state/              # Pure state model, window rules engine (rules.rs)
    ├── dispatch/           # Command bus (Command, Event, Dispatcher)
    ├── layout/             # Pure arrange contract (MasterLayout, MonocleLayout, Registry)
    ├── input/              # Input subsystem (Keybindings, Modifiers, PointerState)
    ├── protocols/          # Protocol delegates (XDG Shell, SHM, Seat, Data Device)
    ├── backend/            # OutputManager, Space RenderManager, TtyBackend (libseat/libinput)
    ├── config/             # LuaJIT runtime (LuaConfig, window_rule, spawn_at_startup)
    └── ipc/                # JSONL UNIX socket IPC server & broadcaster
```

---

## CLI Launch Flags & Subcommands

```bash
# Launch compositor (auto-detects graphical host vs bare TTY)
truss

# Launch with custom config and socket
truss -c ~/.config/truss/my-config.lua -s truss-1

# Force a specific backend
truss --backend winit    # Force nested window
truss --backend tty      # Force direct DRM/libseat TTY
truss --backend headless # Force headless socket mode

# Send IPC commands from terminal or scripts
truss msg state-get
truss msg workspace-switch 2
truss msg close-window
truss msg toggle-floating
truss msg toggle-fullscreen
truss msg layout-set monocle
truss msg set-gap 16
truss msg spawn "kitty"

# Launch the live companion status bar
truss bar
```

---

## Default Keybindings

| Key Combo | Action | Description |
| :--- | :--- | :--- |
| `Super + Return` | `Spawn("kitty")` | Launch terminal emulator |
| `Super + D` | `Spawn("fuzzel")` | Launch application menu |
| `Super + Q` | `WindowClose` | Close focused window |
| `Super + Shift + Q` | `CompositorQuit` | Gracefully shut down compositor |
| `Super + F` | `WindowToggleFullscreen` | Toggle fullscreen on focused window |
| `Super + Shift + Space` | `WindowToggleFloating` | Toggle floating mode on focused window |
| `Super + J` | `WindowFocusDir(Next)` | Focus next window in layout |
| `Super + K` | `WindowFocusDir(Prev)` | Focus previous window in layout |
| `Super + Space` | `WindowSwapMaster` | Swap currently focused window with master |
| `Super + 1..9` | `WorkspaceSwitch(id)` | Switch active workspace (1 through 9) |
| `Super + Shift + 1..9` | `WindowMoveToWorkspace`| Move focused window to workspace (1 through 9) |
| `Super + Left Drag` | `PointerDragMove` | Drag and move window (automatically floats) |
| `Super + Right Drag` | `PointerDragResize` | Drag and resize window (automatically floats) |

---

## Lua Configuration

Place configuration at `~/.config/truss/config.lua`:

```lua
-- Window Rules
truss.window_rule("audio", { app_id = "pavucontrol", floating = true })
truss.window_rule("media", { app_id = "mpv", floating = true })

-- Autostart
truss.spawn_at_startup("truss bar")

-- Event Hooks
truss.on("workspace.switched", function(event)
    -- print("Switched to workspace: " .. tostring(event.id))
end)
```
