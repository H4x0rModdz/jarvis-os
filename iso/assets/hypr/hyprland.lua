-- LilithOS — Hyprland session configuration (ADR 0036).
-- Installed to /usr/share/jarvis/hypr/hyprland.lua and loaded explicitly by
-- /usr/libexec/jarvis-session-compositor. Hyprland 0.57+ (Lua configuration).
--
-- The desktop model is macOS-like: windows FLOAT and stack, with a title bar
-- and traffic-light buttons; the shell's menu bar and dock are layer
-- surfaces. Tiling is one key away for whoever wants it.
--
-- IMPORTANT: this file could not be run where it was written (no GPU on the
-- dev machine — Hyprland refuses to start). Every block is therefore run
-- through `try`, so one wrong option or a plugin that fails to load costs
-- that block only, never the session, and the reason lands in
-- ~/.cache/jarvis-hyprland-config.log for the next iteration.
--
-- Personal overrides: ~/.config/jarvis/hyprland.lua is loaded last, if present.

local home = os.getenv("HOME") or "/tmp"
local errlog = home .. "/.cache/jarvis-hyprland-config.log"
os.execute("mkdir -p '" .. home .. "/.cache' 2>/dev/null")
do local f = io.open(errlog, "w"); if f then f:close() end end

local function try(what, fn)
    local ok, err = pcall(fn)
    if not ok then
        local f = io.open(errlog, "a")
        if f then
            f:write(what .. ": " .. tostring(err) .. "\n")
            f:close()
        end
    end
end

------------------------------------------------------------------- monitors
-- Preferred mode, automatic placement and scale. The Displays panel changes
-- outputs at runtime through wlr-output-management.
try("monitor", function()
    hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })
end)

---------------------------------------------------------------- environment
try("env", function()
    hl.env("XCURSOR_SIZE", os.getenv("XCURSOR_SIZE") or "24")
    hl.env("HYPRCURSOR_SIZE", os.getenv("XCURSOR_SIZE") or "24")
end)

----------------------------------------------------------------- autostart
-- The same script labwc runs: wallpaper, automount, daemons, the shell.
try("autostart", function()
    hl.on("hyprland.start", function()
        hl.exec_cmd("/usr/libexec/jarvis-session-autostart")
    end)
end)

------------------------------------------------------------- look and feel
try("look", function()
    hl.config({
        general = {
            gaps_in     = 4,
            gaps_out    = 8,
            border_size = 1,
            col = {
                active_border   = "rgba(ffffff2e)",
                inactive_border = "rgba(ffffff14)",
            },
            resize_on_border = true,
            allow_tearing    = false,
            layout           = "dwindle",
            -- Floating windows snap to screen edges and to each other.
            snap = { enabled = true, window_gap = 10, monitor_gap = 10 },
        },
        decoration = {
            rounding       = 12,
            rounding_power = 2,
            active_opacity   = 1.0,
            inactive_opacity = 1.0,
            shadow = {
                enabled      = true,
                range        = 28,
                render_power = 3,
                color        = 0x66000000,
            },
            -- This is the glass behind the shell's panels (see layer rules).
            blur = {
                enabled  = true,
                size     = 8,
                passes   = 3,
                vibrancy = 0.22,
                noise    = 0.01,
                popups   = true,
            },
        },
        animations = { enabled = true },
        misc = {
            force_default_wallpaper = 0,
            disable_hyprland_logo   = true,
            disable_splash_rendering = true,
        },
        layers = {
            allow_new_top_layers_over_existing_fullscreen = true,
        },
    })
end)

---------------------------------------------------------------- animations
-- Hyprland's own defaults: windows pop in from 87 % on a spring, leave fast
-- and linear; workspaces fade.
try("animations", function()
    hl.curve("easeOutQuint",   { type = "bezier", points = { {0.23, 1},    {0.32, 1} } })
    hl.curve("easeInOutCubic", { type = "bezier", points = { {0.65, 0.05}, {0.36, 1} } })
    hl.curve("linear",         { type = "bezier", points = { {0, 0},       {1, 1}    } })
    hl.curve("almostLinear",   { type = "bezier", points = { {0.5, 0.5},   {0.75, 1} } })
    hl.curve("quick",          { type = "bezier", points = { {0.15, 0},    {0.1, 1}  } })
    hl.curve("easy",           { type = "spring", mass = 1, stiffness = 238.1191, damping = 24.21279333 })

    hl.animation({ leaf = "global",        enabled = true, speed = 10,   bezier = "default" })
    hl.animation({ leaf = "border",        enabled = true, speed = 5.39, bezier = "easeOutQuint" })
    hl.animation({ leaf = "windows",       enabled = true, speed = 4.79, spring = "easy" })
    hl.animation({ leaf = "windowsIn",     enabled = true, speed = 4.1,  spring = "easy",         style = "popin 87%" })
    hl.animation({ leaf = "windowsOut",    enabled = true, speed = 1.49, bezier = "linear",       style = "popin 87%" })
    hl.animation({ leaf = "fadeIn",        enabled = true, speed = 1.73, bezier = "almostLinear" })
    hl.animation({ leaf = "fadeOut",       enabled = true, speed = 1.46, bezier = "almostLinear" })
    hl.animation({ leaf = "fade",          enabled = true, speed = 3.03, bezier = "quick" })
    hl.animation({ leaf = "layers",        enabled = true, speed = 3.81, bezier = "easeOutQuint" })
    hl.animation({ leaf = "layersIn",      enabled = true, speed = 4,    bezier = "easeOutQuint", style = "fade" })
    hl.animation({ leaf = "layersOut",     enabled = true, speed = 1.5,  bezier = "linear",       style = "fade" })
    hl.animation({ leaf = "fadeLayersIn",  enabled = true, speed = 1.79, bezier = "almostLinear" })
    hl.animation({ leaf = "fadeLayersOut", enabled = true, speed = 1.39, bezier = "almostLinear" })
    hl.animation({ leaf = "workspaces",    enabled = true, speed = 1.94, bezier = "almostLinear", style = "fade" })
    hl.animation({ leaf = "workspacesIn",  enabled = true, speed = 1.21, bezier = "almostLinear", style = "fade" })
    hl.animation({ leaf = "workspacesOut", enabled = true, speed = 1.94, bezier = "almostLinear", style = "fade" })
end)

--------------------------------------------------------------------- input
-- Keyboard layout: the installer's choice, handed over by
-- jarvis-session-launch as XKB_DEFAULT_*.
try("input", function()
    hl.config({
        input = {
            kb_layout  = os.getenv("XKB_DEFAULT_LAYOUT")  or "us",
            kb_variant = os.getenv("XKB_DEFAULT_VARIANT") or "",
            kb_model   = os.getenv("XKB_DEFAULT_MODEL")   or "",
            kb_options = os.getenv("XKB_DEFAULT_OPTIONS") or "",
            follow_mouse = 2,   -- click to focus; the pointer does not steal it
            sensitivity  = 0,
            touchpad = {
                natural_scroll       = true,
                tap_to_click         = true,
                disable_while_typing = true,
                clickfinger_behavior = true,
            },
        },
    })
end)
try("gestures", function()
    hl.gesture({ fingers = 3, direction = "horizontal", action = "workspace" })
end)

---------------------------------------------------------------- shell glass
-- Blur what is behind the shell's translucent surfaces. `ignore_alpha` keeps
-- the blur to the parts that are actually painted: the menu bar's faint scrim
-- and the transparent area of the full-screen popup surfaces stay clear, the
-- glass panels (alpha >= 0.5) are frosted.
try("layer rules", function()
    hl.layer_rule({
        name  = "lilith-glass",
        match = { namespace = "^jarvis-(dock|menu|popover|dialog|launcher|popup|lilith|toast|avatar)$" },
        blur         = true,
        ignore_alpha = 0.4,
    })
    -- These animate themselves (PopupSurface); a compositor fade on top of a
    -- full-screen surface would dim the whole screen for a moment.
    hl.layer_rule({
        name  = "lilith-popups-own-animation",
        match = { namespace = "^jarvis-(menu|popover|dialog|launcher|popup)$" },
        no_anim = true,
    })
    hl.layer_rule({
        name  = "lilith-desktop-hud",
        match = { namespace = "^jarvis-desktop$" },
        no_anim = true,
    })
end)

-------------------------------------------------------------------- windows
try("window rules", function()
    -- macOS-like: every window floats and stacks. Super+T tiles the focused one.
    hl.window_rule({
        name  = "lilith-float-by-default",
        match = { class = ".*" },
        float = true,
    })
    hl.window_rule({
        name  = "fix-xwayland-drags",
        match = { class = "^$", title = "^$", xwayland = true, float = true,
                  fullscreen = false, pin = false },
        no_focus = true,
    })
end)

------------------------------------------------------------------ title bars
-- Hyprland draws no title bars; the hyprbars plugin does. Traffic lights on
-- the left: close, minimize-ish (send to the scratch workspace), maximize.
-- If the plugin does not load (it must match the running Hyprland build
-- exactly), windows simply have no bar: move with Super+drag, close with
-- Super+Q or from the dock.
try("hyprbars", function()
    hl.plugin.load("/usr/lib64/hyprland/libhyprbars.so")
    hl.config({
        plugin = {
            hyprbars = {
                bar_height            = 30,
                bar_color             = "rgba(2c2c2eff)",
                bar_text_size         = 11,
                bar_text_font         = "Inter",
                bar_buttons_alignment = "left",
                bar_padding           = 12,
                bar_button_padding    = 8,
                bar_part_of_window    = true,
                bar_precedence_over_border = true,
            },
        },
    })
    hl.plugin.hyprbars.add_button({ bg_color = "rgb(ff5f57)", fg_color = "rgba(00000099)", size = 13, icon = "",
        action = "hyprctl dispatch 'hl.dsp.window.close()'" })
    hl.plugin.hyprbars.add_button({ bg_color = "rgb(febc2e)", fg_color = "rgba(00000099)", size = 13, icon = "",
        action = "hyprctl dispatch 'hl.dsp.window.move({ workspace = \"special:minimized\" })'" })
    hl.plugin.hyprbars.add_button({ bg_color = "rgb(28c840)", fg_color = "rgba(00000099)", size = 13, icon = "",
        action = "hyprctl dispatch 'hl.dsp.window.fullscreen({ mode = \"maximized\" })'" })
end)

---------------------------------------------------------------- key bindings
local mod = "SUPER"
-- Dispatchers are built lazily: `D.window.close()` only records the call and
-- runs it inside `try`, so a dispatcher that does not exist (or rejects its
-- arguments) in this Hyprland version costs one binding, not the whole file.
local unpack = table.unpack or unpack
local function lazy(path)
    return setmetatable({}, {
        __index = function(_, key)
            local longer = { unpack(path) }
            longer[#longer + 1] = key
            return lazy(longer)
        end,
        __call = function(_, ...)
            local args = { n = select("#", ...), ... }
            return function()
                local fn = hl.dsp
                for _, key in ipairs(path) do fn = fn[key] end
                return fn(unpack(args, 1, args.n))
            end
        end,
    })
end
local D = lazy({})

local function bind(keys, make_dispatcher, opts)
    try("bind " .. keys, function() hl.bind(keys, make_dispatcher(), opts) end)
end

-- Tap Super alone: talk to Lilith (push-to-talk toggle).
bind(mod .. " + SUPER_L", D.exec_cmd("jarvis-voice-ctl toggle"), { release = true })

bind(mod .. " + Return", D.exec_cmd("foot"))
bind(mod .. " + L",      D.exec_cmd("jarvis-lock-ctl lock"))
bind(mod .. " + Q",      D.window.close())
bind("ALT + F4",         D.window.close())
bind(mod .. " + T",      D.window.float({ action = "toggle" }))
bind(mod .. " + F",      D.window.fullscreen({ mode = "maximized" }))
bind(mod .. " + SHIFT + F", D.window.fullscreen())
bind("ALT + Tab",         D.window.cycle_next())
bind("ALT + SHIFT + Tab", D.window.cycle_next({ prev = true }))
-- Bring back windows sent away by the yellow button.
bind(mod .. " + M",       D.workspace.toggle_special("minimized"))

bind(mod .. " + left",  D.focus({ direction = "left" }))
bind(mod .. " + right", D.focus({ direction = "right" }))
bind(mod .. " + up",    D.focus({ direction = "up" }))
bind(mod .. " + down",  D.focus({ direction = "down" }))

for i = 1, 9 do
    bind(mod .. " + " .. i,          D.focus({ workspace = i }))
    bind(mod .. " + SHIFT + " .. i,  D.window.move({ workspace = i }))
end
bind(mod .. " + mouse_down", D.focus({ workspace = "e+1" }))
bind(mod .. " + mouse_up",   D.focus({ workspace = "e-1" }))

-- Move / resize any window with Super + drag (works with or without a bar).
bind(mod .. " + mouse:272", D.window.drag(),   { mouse = true })
bind(mod .. " + mouse:273", D.window.resize(), { mouse = true })

-- Hardware keys: the same wrappers labwc binds (they also post the OSD toast).
local keys = "/usr/libexec/jarvis-keys/"
bind("XF86AudioRaiseVolume",  D.exec_cmd(keys .. "volume-up"),       { locked = true, repeating = true })
bind("XF86AudioLowerVolume",  D.exec_cmd(keys .. "volume-down"),     { locked = true, repeating = true })
bind("XF86AudioMute",         D.exec_cmd(keys .. "volume-mute"),     { locked = true })
bind("XF86AudioMicMute",      D.exec_cmd(keys .. "mic-mute"),        { locked = true })
bind("XF86MonBrightnessUp",   D.exec_cmd(keys .. "brightness-up"),   { locked = true, repeating = true })
bind("XF86MonBrightnessDown", D.exec_cmd(keys .. "brightness-down"), { locked = true, repeating = true })
bind("XF86AudioPlay",         D.exec_cmd(keys .. "media-toggle"),    { locked = true })
bind("XF86AudioPause",        D.exec_cmd(keys .. "media-toggle"),    { locked = true })
bind("XF86AudioNext",         D.exec_cmd(keys .. "media-next"),      { locked = true })
bind("XF86AudioPrev",         D.exec_cmd(keys .. "media-prev"),      { locked = true })

------------------------------------------------------------ user overrides
try("user config", function()
    local path = home .. "/.config/jarvis/hyprland.lua"
    local f = io.open(path, "r")
    if f then
        f:close()
        dofile(path)
    end
end)
