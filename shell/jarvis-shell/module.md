# jarvis-shell

## Purpose

The Jarvis OS desktop shell — a Qt6/QML application that paints the bar,
launcher, control center, and on-screen overlays. Lives in user space, talks
to Lilith via DBus, and (Phase 1b onward) anchors itself to screen edges via
the `wlr-layer-shell` Wayland protocol.

## Boundaries

- jarvis-shell **does not** execute system effects directly. User actions are
  forwarded to Lilith (`com.jarvis.Lilith.Command(...)`) or to the Action Bus
  for direct calls; never to xdg-open/notify-send/etc. ourselves.
- jarvis-shell **does not** manage windows — the compositor does. We *display*
  window state but never call into Smithay.
- jarvis-shell **must** degrade gracefully when Lilith is unreachable: the
  status LED turns red, but the UI keeps working (clock, settings panel).

## Phase 1a (this module, current)

- Qt6 + QML application that opens a normal toplevel window.
- A 64 px translucent bar at the bottom edge.
- Clock (HH:MM:SS) on the left, AI input field in the center, status LED on
  the right.
- Pressing Enter on the input sends `com.jarvis.Lilith.Command(text)`. The
  parsed reply appears in a fading popup above the bar.
- Validates the entire C++ → Qt → DBus → Lilith → Action Bus chain.

## Phase 1b (next)

- Replace the normal toplevel with a `wlr-layer-shell` surface using
  [LayerShellQt][1]. The bar then anchors to the bottom of every output and
  cannot be obscured by ordinary windows.
- Requires a wlroots-based compositor at runtime (sway, labwc, hyprland, or
  eventually our own). WSLg ships Weston which does **not** support
  `wlr-layer-shell`, so this phase will not be testable inside WSL2.

[1]: https://invent.kde.org/plasma/layer-shell-qt

## Out of scope (later phases)

- Launcher overlay (search & app grid)
- Notifications drawer
- Control center / settings panel
- Lock screen
- Multi-output layout

## Build

Out-of-tree build with CMake. Qt 6.4+ required (Ubuntu 24.04 ships 6.4.2):

```bash
cmake -S shell/jarvis-shell -B /tmp/jarvis-shell-build
cmake --build /tmp/jarvis-shell-build -j
/tmp/jarvis-shell-build/jarvis-shell
```

Run requirements: a working Wayland or X11 display. Inside WSL2, WSLg
provides this transparently — no `DISPLAY` setup needed.

## DBus surface

This module is a *client*, not a server. It consumes:

```
com.jarvis.Lilith.Command(string) -> string    // primary input path
com.jarvis.Lilith.Recall(string)  -> string    // for inline fact lookup
```

## Failure modes

| Failure | Behavior |
|---|---|
| Lilith service offline | Status LED red, input disabled with tooltip |
| DBus session bus missing | Window opens with an error banner, no input |
| Command timeout (> 30 s) | Show timeout in reply popup, input re-enabled |
