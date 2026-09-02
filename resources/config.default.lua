-- ==============================================================================
-- Truss Compositor Default Configuration
-- Location: installed to /etc/xdg/truss/config.lua, or copied to
--           ~/.config/truss/config.lua ($XDG_CONFIG_HOME/truss/config.lua)
-- A user configuration file fully replaces this embedded default: settings,
-- keybindings, window rules, autostart and hooks defined here are only active
-- when no user config file exists.
-- ==============================================================================

-- ------------------------------------------------------------------------------
-- 1. Settings
-- ------------------------------------------------------------------------------
truss.set("gap", 8)
truss.set("ratio", 0.55)
truss.set("bg_color", "#14141a")

-- ------------------------------------------------------------------------------
-- 2. Keybindings
-- ------------------------------------------------------------------------------
truss.keybind("SUPER", "Return", truss.cmd.spawn("kitty"))
truss.keybind("SUPER", "d", truss.cmd.spawn("fuzzel || rofi -show drun || wofi"))
truss.keybind("SUPER", "q", truss.cmd.close_window())
truss.keybind("SUPER+SHIFT", "q", truss.cmd.quit())
truss.keybind("SUPER", "f", truss.cmd.toggle_fullscreen())
truss.keybind("SUPER+SHIFT", "space", truss.cmd.toggle_floating())
truss.keybind("SUPER", "j", truss.cmd.window_focus_dir("next"))
truss.keybind("SUPER", "k", truss.cmd.window_focus_dir("prev"))
truss.keybind("SUPER", "space", truss.cmd.swap_master())
for ws = 1, 9 do
    truss.keybind("SUPER", tostring(ws), truss.cmd.workspace_switch(ws))
    truss.keybind("SUPER+SHIFT", tostring(ws), truss.cmd.move_to_workspace(ws))
end

-- ------------------------------------------------------------------------------
-- 3. Window Rules
-- ------------------------------------------------------------------------------
-- Automatically configure properties for applications matching app_id or title
truss.window_rule("audio-control", {
    app_id = "pavucontrol",
    floating = true,
})

truss.window_rule("media-player", {
    app_id = "mpv",
    floating = true,
})

truss.window_rule("image-viewer", {
    app_id = "imv",
    floating = true,
})

truss.window_rule("calc", {
    app_id = "calculator",
    floating = true,
})

truss.window_rule("display-settings", {
    app_id = "wdisplays",
    floating = true,
})

-- ------------------------------------------------------------------------------
-- 4. Autostart Applications (spawned once when compositor is live)
-- ------------------------------------------------------------------------------
-- Set wallpaper (if swaybg / hyprpaper is installed)
-- truss.spawn_at_startup("swaybg -c '#14141a'")

-- Launch status bar (e.g. waybar or truss bar)
truss.spawn_at_startup("waybar")

-- Launch notification daemon
-- truss.spawn_at_startup("mako")

-- ------------------------------------------------------------------------------
-- 5. Event Hooks (Reactive scripting)
-- ------------------------------------------------------------------------------
truss.on("workspace.switched", function(event)
    -- Trigger custom scripts, status bar updates, or notification alerts
    -- print("Switched to workspace: " .. tostring(event.id))
end)

truss.on("window.created", function(event)
    -- print("Window created: " .. tostring(event.id))
end)
