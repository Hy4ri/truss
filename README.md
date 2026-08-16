# truss

A minimal, efficient, keyboard-first dynamic tiling Wayland compositor.

Built on smithay, from scratch — no wlroots.

---

## The Design Process

Every step below is a real decision made in the design grill (2026-08-16).
Each one was argued, weighed, and locked with intent.

### Step 1 — The idea: a compositor from scratch

Wanted: something useful to build, something to get busy with. The target:
a Wayland window manager / compositor, explicitly **not** based on wlroots.

Viability check on the dev box before committing:

- aarch64 host with `/dev/dri` present (`card0`, `renderD128`) — real GPU/display
  hardware access, the thing can actually run, not just pretend
- `rustc` 1.75, `gcc`, `pkg-config`, and the Wayland dev libraries installed
- verdict: a real project with real hardware potential

### Step 2 — Toolkit: smithay

Options considered:

- raw `libwayland-server` in C — hand-rolling KMS, EGL, libinput, the whole
  wire protocol. Maximum masochism, months before a first window
- smithay (Rust) — the independent, non-wlroots compositor toolkit;
  what cosmic-comp and niri are built on

Decided: **smithay**. It handles the plumbing (DRM, EGL, seats, protocol
transport). The compositor brain is still 100% ours: shell handling, layout,
rendering loop, input policy. The engine is built by us — we just don't
forge the bolts.

### Step 3 — Kind: a minimal dynamic tiler

Options considered:

- tiling (sway/i3 style) — windows auto-split, keyboard-driven, no decorations
- floating (mutter style) — server-side decorations, dragging, z-ordering;
  2-3x the work before anything pretty
- hybrid (hyprland style) — both; most impressive, most work

Decided: **dynamic tiling** — dwm-style master + stack auto-layouts, no
manual tile management. Fits the lean philosophy, and there is no famous
dynamic tiler in the smithay ecosystem — a gap worth filling.

Directive from the boss: **minimal and efficient** is the core philosophy.

### Step 4 — Anatomy: 14 parts, 4 families

The compositor was stripped into organs and discussed one by one:

- The Brain — state model, command dispatcher, layout engine, keybind engine
- The Doors — IPC socket, Lua runtime, config
- The Protocol — xdg-shell, surfaces & buffers, rendering, outputs, input
- The Body — session & lifecycle, testing harness

### Step 5 — State model: one source of truth

- a single `State` holds everything: workspaces, windows (id, app_id, title,
  workspace, size, floating flag), focus, per-workspace layouts, keybinds
- nothing lives outside it; no hidden copies, no "the shell keeps its own state"
- mutation **only** through the dispatcher — internal code uses the same
  commands as everything else
- pure data, serializable to JSON: queryable, snapshot-able, debuggable

### Step 6 — Command dispatcher: the API brain

- one bus, named commands: `verb.object` with typed args
  (`workspace.switch{index:3}`, `window.focus{id:12}`, `layout.set{name:"master"}`)
- two doors, one path — keybind, Lua, and IPC socket all hit the same
  parse → validate → execute → broadcast pipeline, with identical errors
- validate **before** mutate: a bad command returns a clean error and state
  is untouched. Atomic or nothing
- every command answers: ok or error. No silent failures
- the command registry is **open** — Lua can register new commands at runtime,
  so plugins don't just call the API, they extend it

### Step 7 — Layout engine: one contract

- one interface: `arrange(windows, area) → rects`
- layouts are pure-ish functions with small params (master count, ratio) —
  trivially testable, trivially fast
- named registry, per-workspace selection
- Lua layouts are first-class: a registered Lua function *is* a layout,
  zero latency, hot-reloadable
- the compositor owns the edges (gaps, borders, floating windows bypass
  layout); layouts own the flow — no layout ever touches pixels
- arrange runs on events (add/remove/switch/resize), never per-frame;
  result cached, damage only what moved
- scope call: **v1 ships `master` only.** The registry stays open;
  more layouts bolt on later

### Step 8 — Keybind engine: keys to commands

- a keybind is `(mods + key) → list of commands` — dumb glue over the dispatcher
- the compositor grabs keys: bound keys never reach clients. This is the
  native Wayland model (sxhkd is X11-only and does not exist here)
- Lua defines them, the API manages them — `keybind.add/remove` at runtime,
  hot-reloadable
- no modes in v1: a mode is just a keybind swap, which is already an API call.
  Core stays dumb, Lua gets fancy
- repeat by default; conflicts resolve last-wins, logged

### Step 9 — IPC socket: the external door

- one unix socket at `$XDG_RUNTIME_DIR/truss.sock`, permissions locked to
  the user — the whole security model, same trust as sway/hyprland
- JSONL: newline-delimited JSON, request/response plus a subscribe event
  stream (`{event:"window.focused", data:{...}}`) with per-client filters
- the socket is just another door to the dispatcher — no logic of its own
- full state is queryable as one JSON tree: debugging is reading a document

### Step 10 — Lua runtime: the scripting door

- mlua (the proven embed: wezterm, mpv, awesomewm)
- Lua sees exactly one `api` table and nothing else — no `os.execute`,
  no arbitrary system calls. Everything touching the outside world goes
  through commands. Purity and safety in one move
- pcall everywhere: a crashy script fails gracefully, the compositor keeps
  breathing. Config error → defaults + a loud log
- `on()` hooks (`on("window.focused", fn)`) turn configs into behavior

### Step 11 — Config: one file, sourcing

- one file: `$XDG_CONFIG_HOME/truss/config.lua`, built-in defaults when absent
- boot pipeline: defaults register → config loads (pcall'd) → registrations
  land → doors open → startup apps launch
- broken config ≠ broken desktop: defaults + loud log + `config.error` event
- `config.reload` re-runs config; live state survives untouched
- **sourcing** (boss's addition): `source("keybinds.lua")`, `source("rules.lua")`,
  `source("layouts.lua")` — one primitive. Relative paths, shared context,
  cycle guard, order matters with later-wins. `main.lua` becomes an
  orchestrator, not a giant sheet. Window rules are just
  `api.rule.add{...}` commands in a sourced file — no separate rules engine

### Step 12 — The name: truss

Candidates: truss, lattice, lean, flux (rejected — fluxbox already owns it
in WM-land).

Decided: **truss** — a civil-engineering structure. Minimal material,
maximum strength, triangles doing all the work. The philosophy, named.

### Step 13 — Home: hyari

- bare git repo at `~/Projects/truss.git` on hyari (LAN host)
- local dev copy at `/opt/truss` tracks `hyari/main`
- build here, push there

---

## Architecture Snapshot

- **State model** — single source of truth; mutation only via dispatcher
- **Command dispatcher** — one bus, named commands, two doors one path,
  validate-before-mutate, open registry
- **Layout engine** — `arrange(windows, area) → rects`; named registry;
  Lua layouts first-class; v1: master only
- **Keybind engine** — (mods + key) → commands; compositor grabs keys;
  API-managed
- **IPC socket** — unix socket, JSONL, request/response + subscribe
- **Lua runtime** — mlua; one api table; pcall everywhere; on() hooks
- **Config** — one config.lua; source() primitive; defaults fallback;
  hot reload
- **Protocol** — xdg-shell, surfaces, rendering, outputs, input (smithay
  plumbing, our policy — details land with their milestones)

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

Design locked 2026-08-16. M1 pending.
