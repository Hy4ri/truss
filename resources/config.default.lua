-- ==============================================================================
-- Truss Compositor Default Configuration
-- Location: ~/.config/truss/config.lua or $XDG_CONFIG_HOME/truss/config.lua
-- ==============================================================================

-- ------------------------------------------------------------------------------
-- 1. Window Rules
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
-- 2. Autostart Applications (spawned once when compositor is live)
-- ------------------------------------------------------------------------------
-- Set wallpaper (if swaybg / hyprpaper is installed)
-- truss.spawn_at_startup("swaybg -c '#14141a'")

-- Launch status bar (e.g. waybar or truss bar)
-- truss.spawn_at_startup("waybar")

-- Launch notification daemon
-- truss.spawn_at_startup("mako")

-- ------------------------------------------------------------------------------
-- 3. Event Hooks (Reactive scripting)
-- ------------------------------------------------------------------------------
truss.on("workspace.switched", function(event)
    -- Trigger custom scripts, status bar updates, or notification alerts
    -- print("Switched to workspace: " .. tostring(event.id))
end)

truss.on("window.created", function(event)
    -- print("Window created: " .. tostring(event.id))
end)
