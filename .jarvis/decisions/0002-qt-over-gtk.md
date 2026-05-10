# ADR 0002: Qt/QML over GTK for UI Framework

## Status
Accepted

## Context

The Jarvis desktop shell needs a UI framework. The main candidates were Qt6/QML and GTK4.

## Decision

Use Qt 6 with QML for the Jarvis shell and design system.

## Reasons

**Qt 6 advantages:**
- Superior animation system with GPU-thread animators
- QML is declarative and highly readable (AI-friendly)
- Excellent Wayland integration via QtWayland
- SceneGraph renderer is GPU-accelerated by default
- Strong commercial ecosystem (proven for desktop applications)
- Rust bindings available (qmetaobject-rs, Slint)
- Better custom rendering support (OpenGL/Vulkan integration)

**GTK4 disadvantages:**
- CSS-based styling is fragile and unpredictable at scale
- Animation support is limited compared to Qt
- Less mature Wayland compositor tooling
- Custom rendering is harder to integrate

## Consequences

- Team must develop QML competency
- LGPL licensing must be reviewed for compliance
- Qt commercial license is not needed for open source project
- Binary size is larger than GTK (acceptable tradeoff)

## Alternatives Considered

- **Electron**: Rejected — far too heavy for a shell, conflicts with philosophy
- **Flutter**: Rejected — Linux desktop support is still immature
- **Custom (wgpu/Winit in Rust)**: Viable long-term, too much reinvention now
