# ADR 0014: Lock Screen — Layer-Shell Overlay + PAM (V1)

## Status
Accepted (V1). V2 lock-with-session-lock-v1 deferred.

## Context

The greeter handles authentication at boot. The lock screen handles
re-authentication during an active session — different surface,
different lifecycle, same broad purpose. labwc + wlroots support the
`ext-session-lock-v1` protocol that gives compositor-level
guarantees against input bypass, but Qt 6.5 doesn't expose that
protocol through QtWayland yet, and writing a hand-rolled
wayland-client integration for one window crosses a complexity bar
we don't want to pay in Phase 3.

V1 needs to ship now. Real compositor-level lock is V2 once we
either bump Qt to a version with session-lock-v1 support, or ship
our own Smithay compositor that gives us the protocol natively.

## Decision

Build a **layer-shell Overlay** lock instead. The lock window:

- Uses `wlr-layer-shell`'s `Overlay` layer (above every other surface
  on the screen, including dialogs).
- Sets `KeyboardInteractivity::Exclusive` so all keyboard input is
  routed to the lock window — Alt+Tab and shortcuts don't leak past.
- Authenticates with PAM via the Rust `pam` crate (same machinery
  `sudo` uses).
- Exposes `com.jarvis.Lock` on the session bus with a single method,
  `Lock()`. The shell calls this when the user clicks the bar's lock
  button or presses Super+L (labwc keybind).

The window is **not** session-lock-v1. A determined attacker can
defeat layer-shell exclusivity by switching VTs (`Ctrl+Alt+F2`) and
killing the daemon. That's a known V1 limitation, documented in the
module.md. V2 fixes it.

## Reasons

- **Ships now.** Layer-shell with Exclusive keyboard already works in
  every compositor in our stack (labwc + the future Jarvis
  compositor). No new Wayland protocol work.
- **Same Qt + LayerShellQt path the shell already uses.** The
  jarvis-shell binary links LayerShellQt for the bar; jarvis-lock
  reuses the same crate / CMake pattern. Zero new external deps.
- **PAM, not a custom hash store.** Logging in to the device is one
  source of truth — pam_unix. The same PAM stack greetd uses at
  boot. Re-implementing auth from scratch for the lock screen would
  be both a security and a usability bug.
- **DBus surface lets the shell + Lilith trigger lock without
  duplicating the activation logic.** Lilith's "lock the screen"
  intent eventually routes through `com.jarvis.Lock.Lock()`.

## Consequences

- New `system/lock/` Rust daemon hosting `com.jarvis.Lock`.
- New `shell/jarvis-lock/` Qt project, sibling of the greeter,
  links `LayerShellQt`. Renders a stripped-down version of the
  Standard mode card (logo + welcome line + password + UNLOCK).
- The shell gains a small lock button on the bar; labwc's rc.xml
  picks up a Super+L keybind that runs `jarvis-lock-ctl lock`
  (tiny CLI mirroring jarvis-voice-ctl's pattern).
- Real PAM dependency (`libpam-devel` in the builder, `libpam` at
  runtime).

## V1 vs V2

| Item | V1 (this) | V2 |
|---|---|---|
| Exclusivity | layer-shell Overlay + KeyboardInteractivity Exclusive | `ext-session-lock-v1` protocol (compositor-level) |
| VT-switch resistance | None (Ctrl+Alt+F2 escapes) | Compositor refuses VT switch while locked |
| Auth methods | PAM password | + biometrics, PIN, Yubikey, voice |
| Idle trigger | Manual lock only | Auto-lock after N minutes idle (settings.idle_lock_after) |
| Multi-output | Single window on primary output | One lock surface per output |

## Alternatives Considered

- **Adopt swaylock and theme it.** Rejected: same trade-offs as
  rejecting an off-the-shelf greeter in ADR 0012. We want the
  Jarvis design language end-to-end and want the integration
  hooks (Lilith trigger, future biometrics) under our roof.
- **Skip V1 and wait for session-lock-v1 support.** Rejected: that
  could mean Phase 4+, and shipping a "lock locally trustable
  enough for daily use" beats nothing in the interim.
- **Implement session-lock-v1 manually via wayland-client crate.**
  Rejected for V1 scope. Worth revisiting in V2 if Qt still
  doesn't expose the protocol by then.
