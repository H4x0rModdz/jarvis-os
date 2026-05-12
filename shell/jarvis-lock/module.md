# jarvis-lock (Qt overlay)

## Purpose

The lock screen UI. Spawned by `jarvis-lock` (the system daemon)
when `com.jarvis.Lock.Lock()` is called. Renders a fullscreen
overlay window via wlr-layer-shell, captures the user's password,
calls `Verify` back on the daemon, and exits when the daemon kills
it after a successful unlock.

This binary deliberately holds no auth state. It collects the
password into a QString, ships it over DBus, and clears it from the
field — the daemon does the PAM call.

See [ADR 0014](../../.jarvis/decisions/0014-lock-screen.md) for
why we use layer-shell Overlay instead of `ext-session-lock-v1` in
V1, and why the overlay is a separate binary from the daemon.

## Boundaries

- The overlay **does not** keep the password. After submit, the
  TextField is cleared. The QString lives only as long as the DBus
  call takes.
- The overlay **does not** decide whether to unlock. It calls
  `com.jarvis.Lock.Verify(password)` and waits — the daemon owns
  the verdict.
- The overlay **does not** survive a daemon restart. The daemon
  spawns and kills it; if the daemon dies, systemd restarts it and
  the overlay process is left orphaned (the daemon clears its
  state, so the next `Lock()` spawns a fresh one).
- The overlay **does not** depend on the shell's QML modules. It
  duplicates a tiny Theme.qml locally — the lock surface must come
  up even if the rest of `jarvis-shell` is broken.

## How It Locks the Session

| Mechanism | Effect |
|---|---|
| `LayerShellQt::Window::LayerOverlay` | Stacks above every other surface on every output. |
| Anchors top + bottom + left + right | Fills the screen. |
| `setExclusiveZone(-1)` | Bar / panels stay visible *under* us (we cover them) but their exclusive zones don't push us. |
| `KeyboardInteractivityExclusive` | Compositor routes every keypress to us; Alt+Tab and the labwc keymap can't bypass. |
| `setScope("jarvis-lock")` | Lets the compositor / debug tools distinguish us from regular layer surfaces. |

VT switch (`Ctrl+Alt+Fn`) still escapes — that's the documented V1
limitation. V2 plus `ext-session-lock-v1` closes that hole.

## Interface (with the daemon)

```
DBus  com.jarvis.Lock  at  /com/jarvis/Lock   (session bus)

  Verify(password: string) -> string   // JSON { ok, reason? }
```

On success the daemon kills this process, so we don't need a clean
"exit on unlock" branch — we just let the QML show a "Verificando…"
state until the kill arrives. On failure we parse the reason out of
the JSON and flash an error message under the field.

## Files

- `shell/jarvis-lock/src/main.cpp` — Qt bootstrap + layer-shell setup.
- `shell/jarvis-lock/src/lock_client.{h,cpp}` — QObject that wraps
  the `com.jarvis.Lock.Verify` DBus call so QML can call it as a
  context property.
- `shell/jarvis-lock/qml/Main.qml` — full-screen overlay layout.
- `shell/jarvis-lock/qml/Theme.qml` — local copy of the Jarvis
  visual tokens (intentionally not shared with `jarvis-shell` so the
  lock window has no runtime dependency on the rest of the shell).
- `shell/jarvis-lock/CMakeLists.txt` — installs to
  `/usr/bin/jarvis-lock-window`, which matches `LOCK_WINDOW_BIN` in
  `system/lock/src/main.rs`.

## Visual Design

V1 mirrors the lock card from `jarvis_login_screen.md`: dark
backdrop, centered glass card with the eye logo, username (read
from `$USER` at startup, not editable from the lock), password
field, UNLOCK button. Same Theme tokens as the greeter so the
visual transition between login → desktop → lock is continuous.

The wallpaper is the same `jarvis-op-default-wallpaper.png` that
ships with the greeter, baked into the binary's resources at
`qrc:/branding/wallpaper.png` so the lock surface has no dependency
on `$XDG_DATA_DIRS` being set.

## Failure Modes

| Failure | Behavior |
|---|---|
| Daemon disappeared before we could call `Verify` | DBus call errors; QML shows "Sistema de bloqueio indisponível"; field stays filled so the user can retry. |
| Wayland compositor doesn't support layer-shell | The `#ifdef JARVIS_HAVE_LAYER_SHELL` block is skipped — the window comes up as a regular toplevel. Not actually a *lock* in that mode; we log and continue rather than refusing to show anything. This only matters under non-labwc compositors (e.g., GNOME), which Jarvis OS doesn't ship. |
| User presses Esc | Ignored. The lock window doesn't expose an "exit without auth" path. |
| QML import fails | `engine.rootObjects().isEmpty()` → process exits 1; daemon's child-wait fires and unlocks (fail-open). Same path as a crash. |
