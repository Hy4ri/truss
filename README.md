# truss

A minimal, efficient, keyboard-first dynamic tiling Wayland compositor.

Built on smithay, from scratch — no wlroots.

## Philosophy

- Minimal: no shadows, no animations, no chrome
- Efficient: damage-tracked redraws, lean dependency tree
- Keyboard-first: every action on a keybind, no mouse required
- API-first: the compositor IS an API. Everything routes through one command dispatcher

## Architecture

### The Brain

- **State model** — one single source of truth: workspaces, windows, focus,
  layouts, keybinds. Pure data, serializable to JSON. Mutation only through
  the dispatcher; nothing lives outside it.
- **Command dispatcher** — one bus, named commands (`verb.object` with typed
  args). Two doors, one path: keybinds, Lua, and the IPC socket all hit the
  same parse → validate → execute → broadcast pipeline. Validate before
  mutate; atomic or nothing; every command answers (ok or error).
  The command registry is open — Lua can register new commands at runtime.
- **Layout engine** — one contract: `arrange(windows, area) → rects`.
  Layouts are pure functions with small params. Named registry, per-workspace
  selection. Lua layouts are first-class. The compositor owns the edges
  (gaps, borders, floating windows); layouts own the flow. Arrange runs on
  events, never per-frame. v1 ships `master` only.
- **Keybind engine** — a keybind is `(mods + key) → list of commands`.
  The compositor grabs keys; bound keys never reach clients. Lua defines
  them, the API manages them (add/remove at runtime). No modes in v1 —
  modes are just keybind swaps, scriptable in Lua.

### The Doors

- **IPC socket** — one unix socket at `$XDG_RUNTIME_DIR/truss.sock`, locked
  to the user. JSONL: request/response plus a subscribe event stream.
  Per-client event filters. The socket is just another door to the
  dispatcher — no logic of its own.
- **Lua runtime** — mlua. Lua sees one `api` table and nothing else
  (no `os.execute`). pcall everywhere: a crashy script fails gracefully,
  the compositor keeps breathing. Config error → defaults + loud log.
  `on()` hooks let config react to WM events.
- **Config** — one file: `$XDG_CONFIG_HOME/truss/config.lua`, with built-in
  defaults when absent. `source("file.lua")` primitive: relative paths,
  shared context, cycle guard. `config.reload` re-runs config; live state
  survives.

### The Protocol (smithay plumbing, our policy)

xdg-shell handling, surfaces & buffers, rendering, outputs, input.
Details land as the milestones are reached.

### The Body

Session & lifecycle, testing harness (headless backend + scripted drives).

## Milestones

- M1 — Scaffold: smithay wired, headless backend runs, event loop alive
- M2 — The Brain: state model + command dispatcher + IPC socket
- M3 — First window: xdg-shell mapped and rendered (headless)
- M4 — Layout engine: master, per-workspace registry
- M5 — Keybinds + Lua config: api table, source(), hot reload
- M6 — Input: pointer, focus, mod-drag move/resize
- M7 — Real output: DRM/KMS on /dev/dri
- M8 — Polish: borders, error surfacing, packaging

## Status

M1 pending. Design grill complete: architecture locked 2026-08-16.
