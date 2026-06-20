# Linux Runtime Skill

## Goal

Understand and correctly use Linux system internals to build a stable, performant, and modern desktop runtime.

## Core Subsystems

### Wayland

- LilithOS is Wayland-first — X11 support is compatibility-only via XWayland
- Use `wl_surface`, `xdg_toplevel`, and `xdg_popup` protocols correctly
- Custom compositor must implement `xdg-shell` at minimum
- Avoid X11-isms: no XGetWindowAttributes, no _NET_WM_* hacks
- Layer-shell protocol for taskbars and overlays
- Use `wlr-screencopy` for screenshot/recording, not X11 grabs

### DBus

- All system services must expose a DBus interface
- Use well-known bus names: `com.jarvis.ActionBus`, `com.jarvis.Lilith`
- Prefer `sd-bus` (systemd's C library) or `zbus` (Rust) for implementation
- Document every method, signal, and property in the interface XML
- Never use DBus for high-frequency data — use shared memory or pipes instead

### systemd

- Jarvis system services should be proper systemd units
- Use `systemd --user` for user-session services
- Use socket activation where applicable for lazy startup
- Never `kill -9` a service — use `systemctl stop` or send SIGTERM

### PipeWire

- All audio routing goes through PipeWire — never ALSA or PulseAudio direct
- Voice pipeline (STT/TTS) must integrate via PipeWire node
- Use `pw-link` equivalents for dynamic routing in the voice assistant

### Process Model

- Each major subsystem should be a separate process (not a thread)
- Use Unix domain sockets or DBus for IPC between processes
- Crash isolation: compositor crashing must not take down the whole session
- Use `seccomp` filters for sandboxed processes (app containers)

### Vulkan / GPU

- All compositor rendering via Vulkan (or wlroots if using wlroots)
- Avoid OpenGL for new rendering code
- Shader compilation must be pre-cached at install time, not at runtime
- GPU memory management: avoid allocating textures every frame

## Permissions Model

- Follow least-privilege: services request only what they need
- Use Linux namespaces for application sandboxing
- Flatpak portals for cross-boundary file/media access
- Never give AI assistant root — use polkit for privileged operations

## Performance Baselines

- Compositor frame time: < 2ms on modern GPU
- DBus call round-trip: < 1ms for local calls
- Service startup: < 100ms for user-session services
- Idle CPU for desktop: < 0.5%
