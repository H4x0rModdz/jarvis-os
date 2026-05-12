# jarvis-shell

## Purpose

The Jarvis OS desktop shell — a Qt6/QML application that paints the
always-present bar at the bottom of the screen, the launcher overlay,
the permission approval dialog, and the first-boot updater splash.
Lives in user space, talks to the system daemons via DBus, anchors
itself to screen edges via the `wlr-layer-shell` Wayland protocol.

## Boundaries

- jarvis-shell **does not** execute system effects directly. User
  actions go through `com.jarvis.Lilith.Command` (Lilith path) or
  `com.jarvis.ActionBus.Dispatch` (direct path — e.g. the launcher
  clicking an app tile). We never shell out to xdg-open / notify-send /
  pactl ourselves; those are Action Bus handler concerns.
- jarvis-shell **does not** manage windows. The compositor does. We
  display window state via DBus events but never call into Wayland
  ourselves.
- jarvis-shell **must** degrade gracefully when any daemon is offline:
  the Lilith status LED turns red, the bar still renders, the clock
  still ticks, the launcher still opens (it dispatches via the Action
  Bus directly, not Lilith).

## Components

| QML / C++ | Role |
|---|---|
| `qml/Main.qml` | Root layer-shell window holding bar, reply popup, and sibling dialogs. |
| `qml/Bar.qml` | The bar surface — hamburger, clock, Lilith text input, status LED. |
| `qml/Launcher.qml` | Overlay with search + 4-column app grid backed by `DesktopAppsModel`. |
| `qml/ApprovalDialog.qml` | Modal-ish window bound to `PermissionBridge.hasPending`. |
| `qml/UpdaterSplash.qml` | First-boot splash bound to `UpdaterBridge.active`. |
| `qml/Theme.qml` | Singleton design tokens (colors, radii, animation durations). |
| `src/lilith_bridge.{h,cpp}` | `com.jarvis.Lilith.Command` + `Recall`. |
| `src/permission_bridge.{h,cpp}` | Subscribes to `ApprovalRequested`, exposes pending state + `ResolveApproval`. |
| `src/action_bus_bridge.{h,cpp}` | Direct `Dispatch` for shell-internal callers (launcher). |
| `src/updater_bridge.{h,cpp}` | Subscribes to updater `Progress` + `Completed` signals. |
| `src/desktop_apps_model.{h,cpp}` | `QAbstractListModel` over XDG `applications/` dirs. |

## DBus Surface

This module is a *client*, not a server. It consumes:

```
com.jarvis.Lilith.Command(text)         primary input path (bar)
com.jarvis.Lilith.Recall(key)           inline fact lookup
com.jarvis.ActionBus.Dispatch(json)     direct dispatch (launcher tile click)
com.jarvis.PermissionSystem
  signal ApprovalRequested(...)         drives ApprovalDialog
  method ResolveApproval(id, decision)  user button press
com.jarvis.Updater
  signal Progress(stage, percent, msg)  drives UpdaterSplash
  signal Completed(success, message)    dismiss / failure state
```

## Build

Out-of-tree CMake. Qt 6.5+ required (the QML singleton mechanism Theme
depends on misbehaves under the `REQUIRES 6.4` compat mode — see ADR
notes inside the commit that fixed it). On Fedora 42 in the ISO build
the Qt is 6.10; on Ubuntu 24.04 you'll need to install Qt 6.5+ via
`aqtinstall` (`tools/dev/run-shell-labwc.sh` documents the path).

```bash
cmake -S shell/jarvis-shell -B /tmp/jarvis-shell-build \
      -DCMAKE_BUILD_TYPE=Release
cmake --build /tmp/jarvis-shell-build -j
/tmp/jarvis-shell-build/jarvis-shell
```

Run requirements: a working Wayland or X11 display. Under WSL2 + WSLg
the X11 path is fine for development. The layer-shell anchor only kicks
in if `LayerShellQt` was found at configure time; otherwise the shell
falls back to a regular xdg-shell toplevel (same UI, just floats).

## Failure modes

| Failure | Behavior |
|---|---|
| Lilith service offline | Status LED red, bar still accepts input but `Command` errors are surfaced in the reply popup. |
| Permission daemon offline | `ApprovalDialog` never opens; dangerous-scope dispatches fail via the Action Bus's local fallback. |
| Updater daemon offline | `UpdaterSplash` never opens; if the model is missing, Lilith stays offline and the user is on their own. |
| DBus session bus missing | The shell logs an error and runs with empty bridges — clock + launcher still work. |
| Command timeout (> 30 s) | Reply popup shows the timeout, input re-enabled. |

## Out of scope (later phases)

- Notifications drawer (Phase 2)
- Control center / settings panel (Phase 2)
- Lock screen (Phase 2)
- Multi-output layout polish (Phase 3, alongside the custom compositor)
- Glassmorphism shader pass (Phase 3 — requires our compositor for proper backdrop blur)
