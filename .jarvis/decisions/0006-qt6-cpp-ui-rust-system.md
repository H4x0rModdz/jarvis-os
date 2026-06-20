# ADR 0006: Qt6/C++ for Shell UI, Rust for Everything Else

## Status
Accepted

## Context

LilithOS needs a UI framework for the desktop shell. The two realistic options were:
- Qt6/QML (C++) — mature, proven for desktop shells, excellent animation/GPU support
- Slint (Rust) — full Rust, growing ecosystem, less mature for complex desktop shells

## Decision

**Qt6/QML for all shell UI components. Rust for all system daemons and the compositor.**

The language boundary is explicit:

| Layer | Language | Rationale |
|---|---|---|
| Shell UI (taskbar, launcher, overlays, control center) | C++ / QML | Qt6 is the best desktop UI stack on Linux |
| Compositor (Wayland server, window management, rendering) | Rust (Smithay) | Pure Rust, proven by COSMIC DE |
| System daemons (Action Bus, Permission System, Lilith, Automation) | Rust | Type safety, performance, async |
| SDK bindings | Rust primary, C++ secondary | Match the caller's ecosystem |

## Architecture Implication

The compositor and shell UI are separate processes:

```
jarvis-compositor  (Rust/Smithay) ← Wayland server
      ↑ Wayland protocol
jarvis-shell       (C++/Qt6/QML)  ← Wayland client (layer-shell for taskbar/overlays)
```

The shell is a privileged Wayland client, not part of the compositor.
This is the same pattern used by KDE Plasma (KWin + Plasmashell).

## Interop Boundary

Rust daemons ↔ Qt shell communicate via DBus:
- Shell calls Action Bus via `com.jarvis.ActionBus`
- Shell receives Lilith responses via `com.jarvis.Lilith` signals
- Shell shows permission dialogs when `PermissionRequired` signal fires

No shared memory, no FFI, no direct function calls across the boundary.
DBus is the contract.

## Reasons for Qt6 over Slint

- Qt6 is battle-tested for full desktop environments (KDE Plasma, LXQt)
- QML animation system and SceneGraph are mature and GPU-accelerated
- Wayland/layer-shell integration is first-class in Qt6
- Larger ecosystem and community
- Lower development risk at this stage

## Reasons This Is Acceptable Despite C++

- The C++ surface is isolated to one module: `shell/`
- All business logic, permissions, AI, and system interactions stay in Rust
- The DBus boundary keeps the two codebases cleanly separated
- Qt6 LGPL license is compatible with open source

## Consequences

- `shell/` is a C++/QML codebase with its own CMake build system
- All other modules remain pure Rust with Cargo
- Root build orchestration needs to handle both Cargo and CMake
- Contributors to `shell/` need Qt/QML knowledge; system contributors need Rust
