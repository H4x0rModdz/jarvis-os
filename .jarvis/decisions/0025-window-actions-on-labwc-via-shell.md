# 0025 — Wire window.{focus,minimize,maximize,close} to labwc via the shell (revises 0024)

Status: accepted
Date: 2026-06-02

## Context

ADR 0024 deferred **all** `window.*` / `workspace.*` Action Bus actions to
the future Smithay compositor, so today they return `UNAVAILABLE`. That was
overly broad. The `wlr-foreign-toplevel-management-v1` protocol — which
labwc implements and the dock already consumes (`shell/jarvis-shell/src/
foreign_toplevel.cpp`) — exposes write requests:

- `activate(seat)` (focus), `set_minimized` / `unset_minimized`,
  `set_maximized` / `unset_maximized`, `close`.

So `window.focus` / `minimize` / `maximize` / `close` are reachable on
labwc **today**, with the exact same protocol the Smithay compositor will
implement — the work carries over unchanged. What the protocol does **not**
expose is geometry: there is no move-to-coords, resize-to-size, or snap, and
labwc offers no external IPC for those or for workspaces. Those stay
deferred.

## Decision

1. **The shell serves window control over DBus.** The shell is already the
   de-facto window manager (it owns the foreign-toplevel client, ADR 0024).
   It registers a service `com.jarvis.Shell`, object `/com/jarvis/Shell`,
   interface `com.jarvis.Shell.Windows`, with:
   - `Focus(s target) -> b`, `Minimize(s target) -> b`,
     `Maximize(s target) -> b`, `Close(s target) -> b`
   - `List() -> s` (JSON array of `{app_id, title, activated, minimized}`)

2. **action-bus forwards.** `window.{focus,minimize,maximize,close}` proxy
   the call to `com.jarvis.Shell`. The daemon does NOT open its own Wayland
   client — one foreign-toplevel connection, in one place, reusing tested
   code. Direction is daemon → shell, justified because the shell plays the
   compositor role here.

3. **Selector contract change.** The AI-facing `window.*` tools take a
   string `target` instead of integer `window_id`:
   - `"active"` / `"focused"` → the currently activated window (default),
   - otherwise an app name (matched against `app_id`, normalised like the
     dock: `org.mozilla.firefox` ↔ `firefox`) or a title substring.

   Foreign-toplevel has no stable integer id, and a name/active selector is
   what natural language produces ("minimiza o firefox", "fecha essa
   janela"). `window_id` is dropped from these four tools.

4. **Geometry + workspaces stay deferred.** `window.{move,resize,snap_left,
   snap_right}` and `workspace.*` return `UNAVAILABLE` with an accurate
   "needs the Jarvis compositor" message — not a fake success.

## Consequences

- 4 of the 11 dormant window/workspace actions light up; Lilith can focus /
  minimize / maximize / close windows by voice or text on labwc.
- When the Smithay compositor lands it registers real `window.*` handlers in
  the bus Registry (built-ins win, see `main.rs`), at which point this
  shell-served path is removed and the full geometry/workspace surface
  becomes available — no AI-facing contract change beyond adding the
  deferred verbs back.
- Verification needs a real labwc session (VM/ISO): the Wayland round trip
  isn't exercised by unit tests. The Rust forwarding and selector parsing
  are unit-testable; the C++ request path is validated on the image.

## Alternatives rejected

- **Own `wlr-foreign-toplevel` client in action-bus** — "purer" per ADR
  0004, but duplicates the shell's tested client and adds wayland-client
  boilerplate + a second connection for no user benefit.
- **Keep everything deferred (ADR 0024 as-is)** — leaves the signature
  AI-native capability dark when four verbs are trivially reachable on the
  protocol labwc already speaks.
- **Stable integer window ids via the shell** — would keep the `window_id`
  schema but forces a list-then-pick round trip and an id map the protocol
  doesn't provide; a name/active selector is simpler and more natural.
