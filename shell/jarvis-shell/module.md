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

## Phase 1a (done)

- Qt6 + QML application that opens a normal toplevel window.
- A 64 px translucent bar at the bottom edge (manually centered).
- Clock (HH:MM:SS) on the left, AI input field in the center, status LED on
  the right.
- Pressing Enter on the input sends `com.jarvis.Lilith.Command(text)`. The
  parsed reply appears in a fading popup above the bar.
- Validates the entire C++ → Qt → DBus → Lilith → Action Bus chain.

## Phase 1b (done — wlr-layer-shell anchoring)

- Optionally links against [LayerShellQt][1] (KDE's Qt6 binding for the
  `wlr-layer-shell` protocol). When the library is found at configure time,
  the shell anchors itself to the **bottom** of every output, sits on the
  **Top** layer, and reserves its full height as **exclusive zone** so other
  windows never overlap.
- `LayerShellQt::Shell::useLayerShell()` is called before `QGuiApplication`
  so Qt's Wayland plumbing picks the layer-shell integration plugin at
  startup. If the compositor doesn't speak the protocol (e.g. plain
  GNOME/Mutter or WSLg's Weston), Qt falls back to xdg-shell and the bar
  appears as a regular floating window — Phase 1a behavior, no crash.
- The `LayerShellQt_FOUND` CMake conditional keeps this a non-mandatory
  dependency. CI and slim builds work without it.
- KDE's LayerShellQt requires Qt 6.6+; Ubuntu 24.04 ships only 6.4.2, so we
  install Qt 6.8.3 via `aqtinstall` into `~/Qt/6.8.3/gcc_64` and build
  LayerShellQt v6.0.0 against it (see `tools/dev/run-shell-labwc.sh`).

[1]: https://invent.kde.org/plasma/layer-shell-qt

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
