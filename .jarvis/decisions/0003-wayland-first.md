# ADR 0003: Wayland-First Display Protocol

## Status
Accepted

## Context

LilithOS needs to choose between X11 and Wayland as the primary display protocol.

## Decision

Wayland-first. X11 support only via XWayland for legacy application compatibility.

## Reasons

- Wayland provides smoother rendering with no screen tearing
- Wayland's security model is fundamentally better (apps can't spy on each other's input)
- Wayland enables better compositor control (explicit sync, per-surface frame timing)
- Qt6 has first-class Wayland support
- X11 is in maintenance mode — future development is Wayland
- Modern Linux systems (Fedora, Ubuntu, Arch) already default to Wayland
- Glassmorphism and custom compositor effects are simpler to implement correctly under Wayland

## Consequences

- Some legacy X11-only applications will require XWayland bridge (automatic, transparent to users)
- Global shortcut APIs differ under Wayland — we provide our own portal
- Screen capture / sharing uses Wayland protocols (xdg-desktop-portal) not X11 grabs
- Some anti-cheat systems that depend on X11 input hooks will not function (expected limitation)

## Alternatives Considered

- **X11 primary**: Rejected — older protocol, worse security, screen tearing issues, no future
- **Both equally**: Rejected — doubles maintenance burden, impossible to maintain consistency
