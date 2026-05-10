# Known Limitations

## By Design (Won't Fix)

### Kernel-Level Anti-Cheat
Games using kernel-level anti-cheat (Easy Anti-Cheat kernel mode, BattlEye kernel mode, Vanguard) will not work.
This is a Linux limitation, not a Jarvis limitation. User-mode AC works fine.

### Windows Kernel Drivers
Apps that install Windows kernel drivers will not function.
This includes some hardware peripherals that ship Windows-only drivers.

### Proprietary DRM with Kernel Hooks
Some DRM systems (older Denuvo, StarForce) hook into Windows kernel internals.
Compatibility is not guaranteed and will not be a priority.

## Platform Constraints (May Improve)

### Wayland Adoption
Some older Linux applications are X11-only. XWayland provides compatibility, but some features (global shortcuts, screen capture) may behave differently.

### Ollama Model Quality
Local LLM quality depends on the user's hardware. On low-RAM systems, smaller/less capable models will be used, which affects Lilith's reasoning quality.

### Flatpak Sandbox Depth
Flatpak sandboxing limits what Lilith can observe and act on within sandboxed apps without explicit portals.

## Known Open Questions

- Whether to use wlroots or implement a compositor from scratch
- TTS voice quality vs. local model size tradeoff
- Whether the AI memory store should be encrypted at rest
- Linux distribution base selection (Fedora/Arch/NixOS)

## Not In Scope (Explicitly)

- Mobile/tablet support (desktop-only for now)
- ARM architecture (x86_64 first)
- Custom kernel (Linux base for foreseeable future)
- macOS compatibility layer
- Enterprise MDM / fleet management (future consideration)
