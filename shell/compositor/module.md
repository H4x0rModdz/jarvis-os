# Module: compositor

## Purpose

The Jarvis compositor is the Wayland server. It manages all display surfaces, window
lifecycle, input routing, and GPU rendering. It is the lowest-level component of the
Jarvis desktop — everything visible on screen passes through here.

Built on Smithay (pure Rust Wayland compositor library), using the same proven foundation
as System76's COSMIC desktop.

## Architecture

```
jarvis-compositor  (this module — Rust/Smithay)
      ↑ Wayland protocol (unix socket)
jarvis-shell       (Qt6/QML — taskbar, launcher, overlays via layer-shell)
other apps         (normal Wayland clients via xdg-shell)
      ↕ DBus
jarvis-action-bus  (window.* actions dispatched here)
```

## Exposes

### Wayland Protocols
- `xdg-shell` — standard window management for applications
- `wlr-layer-shell` — anchored surfaces for jarvis-shell (taskbar, overlays)
- `xdg-output` — output geometry for HiDPI and multi-monitor
- `wl-seat` — keyboard and pointer input

### DBus Interface
None exposed directly. Consumes `com.jarvis.ActionBus` to receive `window.*` and
`workspace.*` action requests.

### Internal Window Events → Action Bus
Emits events (via DBus) when windows open, close, or change focus so Lilith stays aware
of desktop state.

## Depends On

- `smithay` — Wayland compositor library
- `com.jarvis.ActionBus` — receives window action commands
- `wlroots` — NOT used (Smithay replaces it)

## Permissions Required

- DRM/KMS access (via libseat for rootless operation)
- Framebuffer access
- Input device access (evdev)

## AI Integration Notes

Lilith controls windows by dispatching `window.*` actions to the Action Bus.
The compositor receives these via a calloop channel (bridged from tokio/DBus).
Lilith does NOT have direct access to Wayland surfaces.

## Performance Characteristics

- Target frame time: < 2ms on integrated GPU, < 1ms on discrete GPU
- Target: 60fps at 1080p, 120fps on high-refresh displays
- Latency-sensitive: yes — this is the rendering hot path

## Known Limitations

- udev/DRM backend (real hardware) is a skeleton — full implementation in Phase 2
- Vulkan renderer deferred to Phase 2 (using OpenGL/GlesRenderer for now)
- No VRR/adaptive sync support yet
- Single monitor only in Phase 1
