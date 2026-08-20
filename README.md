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
│   └── config.default.lua  # Embedded default config; complete reference for user configs
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

> These are the **default** bindings, defined in `resources/config.default.lua`. Keybindings are fully config-driven — nothing is hardcoded in the binary — so every binding below can be redefined or removed in your own config (see [Lua Configuration](#lua-configuration)).

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

Configuration is a single Lua script evaluated by the embedded LuaJIT runtime. The global `truss` table exposes settings, keybindings, window rules, autostart commands, and event hooks. Larger configs can be split into modules with `truss.source("path/to/file.lua")`.

### Config Resolution

Only the first match is loaded — a user configuration file fully replaces the system default:

1. `-c/--config <path>` CLI flag — an explicit path wins unconditionally
2. `$XDG_CONFIG_HOME/truss/config.lua`, falling back to `~/.config/truss/config.lua`
3. `/etc/xdg/truss/config.lua` (system-wide default)
4. The built-in embedded default (a copy of `resources/config.default.lua`) — truss always works with zero configuration

Install the system-wide default with:

```bash
sudo install -Dm644 resources/config.default.lua /etc/xdg/truss/config.lua
```

The complete reference is `resources/config.default.lua` — copy it to `~/.config/truss/config.lua` and edit:

```bash
cp resources/config.default.lua ~/.config/truss/config.lua
```

Alternatively, `truss init-config` writes the default configuration to the user config path (`$XDG_CONFIG_HOME/truss/config.lua`, falling back to `~/.config/truss/config.lua`) and refuses to overwrite an existing file.

### Settings

```lua
truss.set("gap", 8)              -- gap between windows (pixels)
truss.set("ratio", 0.55)         -- master area ratio (0.0 - 1.0)
truss.set("bg_color", "#14141a") -- background color, hex: #rgb or #rrggbb
```

### Keybindings

```lua
truss.keybind("SUPER+SHIFT", "q", truss.cmd.quit())
truss.keybind("SUPER", "d", truss.cmd.spawn("fuzzel"))
truss.keybind("SUPER", "v", "kitty") -- a plain string spawns a shell command
```

- **Modifiers**: `CTRL`, `ALT`, `SHIFT`, `SUPER` — case-insensitive, joined with `+` (`MOD4` and `LOGO` are accepted aliases for `SUPER`).
- **Keys**: single characters (`a`-`z`, `0`-`9`) or named keys: `Return`, `Escape`, `Tab`, `Space`, `BackSpace`, `Delete`, `Insert`, `Home`, `End`, `Page_Up`, `Page_Down`, the arrow keys (`Left`, `Up`, `Right`, `Down`), and `F1`-`F12`.
- **Action**: any `truss.cmd.*` constructor, or a plain string interpreted as a shell command to spawn.

| Constructor | Description |
| :--- | :--- |
| `truss.cmd.workspace_switch(id)` | Switch to workspace `id` |
| `truss.cmd.window_focus_dir("next" \| "prev")` | Focus next or previous window in the layout |
| `truss.cmd.swap_master()` | Swap the focused window with the master |
| `truss.cmd.close_window([id])` | Close the focused window (or the window with id `id`) |
| `truss.cmd.toggle_floating([id])` | Toggle floating on the focused window (or the window with id `id`) |
| `truss.cmd.toggle_fullscreen([id])` | Toggle fullscreen on the focused window (or the window with id `id`) |
| `truss.cmd.move_to_workspace(ws)` | Move the focused window to workspace `ws` |
| `truss.cmd.spawn("cmd")` | Spawn a shell command |
| `truss.cmd.set_gap(n)` | Set the window gap to `n` pixels |
| `truss.cmd.set_ratio(f)` | Set the master ratio to `f` |
| `truss.cmd.quit()` | Gracefully shut down the compositor |

### Window Rules

```lua
truss.window_rule("audio", { app_id = "pavucontrol", floating = true })
truss.window_rule("media", { app_id = "mpv", floating = true })
```

### Autostart

```lua
truss.spawn_at_startup("truss bar") -- deferred until the compositor is live
```

### Event Hooks

```lua
truss.on("workspace.switched", function(event)
    -- print("Switched to workspace: " .. tostring(event.id))
end)
```
