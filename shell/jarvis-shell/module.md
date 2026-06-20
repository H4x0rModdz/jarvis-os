# jarvis-shell

## Purpose

The LilithOS desktop shell — a Qt6/QML application that paints the
macOS-style **top menu bar**, the floating **bottom dock** (app tiles +
the Lilith orb), the **desktop icons**, the launcher overlay, permission
approval dialogs, the first-boot updater splash, the preferences panel,
notification toasts + drawer, and Lilith's conversation popup. Lives in
user space, talks to every Jarvis daemon via DBus, and anchors its three
top-level surfaces to screen edges via the `wlr-layer-shell` Wayland
protocol. See ADR 0022 for the menu-bar/dock/orb layout decision.

## Boundaries

- jarvis-shell **does not** execute system effects directly. User
  actions route through `com.jarvis.Lilith.Command` (Lilith path) or
  `com.jarvis.ActionBus.Dispatch` (direct path — e.g. the launcher
  clicking an app tile). We never shell out to xdg-open / notify-send
  / pactl ourselves; those are Action Bus handler concerns.
- jarvis-shell is the **de-facto window manager on labwc** (ADR 0024/0025).
  It speaks `wlr-foreign-toplevel-management-v1` directly: the dock reflects
  running windows, and `WindowControlService` serves `com.jarvis.Shell` so
  the Action Bus can focus/minimize/maximize/close windows. Geometry,
  snapping and workspaces stay with the future Smithay compositor — when it
  lands it registers real `window.*` handlers and this Wayland path is
  removed.
- jarvis-shell **must** degrade gracefully when any daemon is offline:
  the Lilith LED turns red, but the bar still renders, the clock still
  ticks, the launcher still opens (it dispatches via the Action Bus
  directly, not Lilith).

## Components

The Qt module is registered as `Jarvis.Shell`. Components are split
across `src/` (C++ bridges) and `qml/`.

### Top menu bar

| File | Role |
|---|---|
| `qml/Main.qml` | Root layer-shell window (objectName `jarvis-topbar`, anchored top). Holds `TopBar` + every sibling popup Window (JarvisMenu, AboutDialog, Launcher, ApprovalDialog, UpdaterSplash, SettingsPanel, DisplayPanel, NotificationToast/Drawer, LilithPopup, ConnectivityPanel, BluetoothPanel, FirstBootWizard). Listens to `ShellBus` for intents from the dock. |
| `qml/TopBar.qml` | The menu-bar content — Jarvis logo (left, opens the Jarvis menu) + reused wifi/bluetooth/battery/bell/gear indicators + clock (right). |
| `qml/JarvisMenu.qml` | Apple-menu analogue dropping from the logo: Sobre, Configurações, Atualização, Bloquear/Suspender/Reiniciar/Desligar (the last four dispatch `system.power`). |
| `qml/AboutDialog.qml` | "Sobre este PC" card. |
| `qml/BarGearButton.qml` | Gear glyph (opens the settings panel). |
| `qml/BarBellButton.qml` | Bell glyph (opens the notification drawer). |
| `qml/Clock.qml` | HH:MM:SS time, updates every second. |
| `qml/Theme.qml` | Singleton design tokens (colors, radii, animation durations). |
| `qml/ShellBus.qml` | Singleton signal bus carrying cross-surface UI intents (dock → top-bar popups): `toggleLilith` / `openLauncher` / `openSettings` / `openNotifications`. |

### Dock

| File | Role |
|---|---|
| `qml/Dock.qml` | Floating bottom dock (objectName `jarvis-dock`): a glass pill of pinned app tiles + a divider + the Lilith orb. `main.cpp` anchors it to the bottom edge only (compositor-centered), Top layer, no exclusive zone — maximized windows float under it. App tiles dispatch `app.open`; the Launchpad tile and orb route through `ShellBus`. |
| `qml/DockIcon.qml` | One dock tile — theme/path icon with hover magnification + monogram fallback. |
| `qml/LilithOrb.qml` | Lilith's dock presence. Glyph encodes state — `◉` idle, `◎` listening, `◌◌◌` thinking, `◉◉◉` speaking (from `VoiceBridge.state` + `LilithBridge.busy`). Click → `ShellBus.toggleLilith`; press-and-hold → `VoiceBridge.toggle` (push-to-talk). Subsumes the retired status LED + mic button. |

> **Retired:** `Bar.qml` (the old single bottom bar) and its bar-only
> `LilithInput` / `MicButton` / `StatusIndicator` / `BarMenuButton` are
> no longer instantiated (see ADR 0022). The files remain in the module
> until a follow-up cleanup removes them.

### Desktop surface

| File | Role |
|---|---|
| `qml/Desktop.qml` | Second top-level Window (objectName `jarvis-desktop`) holding the desktop icon column. `main.cpp` anchors it to all four output edges on the wlr-layer-shell *bottom* layer — above swaybg's wallpaper, below app windows — with no keyboard focus. Icons activate via `app.open`: Computador → `computer:///`, Lixeira → `trash:///` (both pinned to Dolphin in `iso/assets/xdg/mimeapps.list`), Pasta Pessoal → the `HomePath` context property. |
| `qml/DesktopIcon.qml` | One labelled desktop icon. Single click selects (accent highlight), double click activates. Theme icon via `image://theme/`, monogram fallback, outlined label for legibility over any wallpaper. |

### Overlays

| File | Role |
|---|---|
| `qml/Launcher.qml` | App-grid overlay with search, backed by `DesktopAppsModel`. |
| `qml/AppCell.qml` | Single app tile inside the launcher grid. |
| `qml/ApprovalDialog.qml` | Modal-ish window driven by `PermissionBridge.hasPending`. |
| `qml/ApprovalButton.qml` | Pill button used inside ApprovalDialog. |
| `qml/UpdaterSplash.qml` | First-boot + OS-upgrade splash, three states (active / os-prompt / reboot). |
| `qml/SettingsPanel.qml` | Preferences window with the live values from `SettingsBridge`. |
| `qml/NotificationToast.qml` | Bottom-right toast for incoming notifications, including action buttons. |
| `qml/NotificationDrawer.qml` | Right-edge drawer listing recent history (newest first). |

### Bridges (C++ singletons exposed as `Jarvis.Shell.*`)

| Bridge | Talks to | What it does |
|---|---|---|
| `LilithBridge` | `com.jarvis.Lilith` | `Command(text)` + ping for reachability + busy state. |
| `PermissionBridge` | `com.jarvis.PermissionSystem` | Subscribes to `ApprovalRequested`, exposes pending state, sends `ResolveApproval`. |
| `ActionBusBridge` | `com.jarvis.ActionBus` | Direct `Dispatch(json)` for shell-internal callers (launcher tile click). |
| `UpdaterBridge` | `com.jarvis.Updater` | Progress + Completed + OSUpdateAvailable signals, `applyOSUpgrade()`. |
| `VoiceBridge` | `com.jarvis.Voice` | `toggle()`, subscribes to StateChanged + TranscriptionFinal. |
| `SettingsBridge` | `com.jarvis.Settings` | Sync getters + async setters + Changed signal re-emit. |
| `NotificationsBridge` | `com.jarvis.Notifications` + `org.freedesktop.Notifications` | Re-emit NotificationPosted, `invokeAction(id, key)`, history list. |
| `DesktopAppsModel` | XDG `applications/` dirs | Scan + filter for the launcher. |

## DBus Surface

It **serves** one interface (window management on labwc, ADR 0025):

```
com.jarvis.Shell  /com/jarvis/Shell  iface com.jarvis.Shell.Windows
  method Focus(target) / Minimize(target) / Maximize(target) / Close(target) -> bool
  method List() -> json   [{app_id,title,activated,minimized}]
  // target: "active"/"focused" | app name | title substring
  // backed by WindowControlService (its own wlr-foreign-toplevel client)
```

As a *client*, it consumes:

```
com.jarvis.Lilith.Command(text)             primary input path (bar)
com.jarvis.Lilith.Recall(key)               inline fact lookup
com.jarvis.ActionBus.Dispatch(json)         direct dispatch (launcher tile click)
com.jarvis.PermissionSystem
  signal ApprovalRequested(...)             drives ApprovalDialog
  method ResolveApproval(id, decision)      user button press
com.jarvis.Updater
  signal Progress(stage, percent, msg)      drives UpdaterSplash progress bar
  signal Completed(success, message)        dismiss / reboot prompt
  signal OSUpdateAvailable(version)         shows the os-prompt mode
  method ApplyOSUpgrade()                   user-clicked Install
com.jarvis.Voice
  signal StateChanged(state)                drives MicButton visual
  signal TranscriptionFinal(text)           piped into LilithBridge.send
  method StartListening / StopListening
com.jarvis.Settings
  method Get / Set / Delete / List
  signal Changed(key, value_json)           valueChanged(key) bubble
org.freedesktop.Notifications
  signal NotificationPosted(id, ...)        drives toast + drawer
  method InvokeAction(id, key)              from action-button click
com.jarvis.Notifications
  method RecentNotifications(limit)         drawer fetch
```

## Build

Out-of-tree CMake. Qt 6.5+ required (the QML singleton mechanism
Theme depends on misbehaves under the `REQUIRES 6.4` compat mode).
Fedora 42 ships Qt 6.10 which is what the ISO build uses; local dev
on Ubuntu 24.04 needs Qt 6.5+ via aqtinstall.

```bash
cmake -S shell/jarvis-shell -B /tmp/jarvis-shell-build \
      -DCMAKE_BUILD_TYPE=Release
cmake --build /tmp/jarvis-shell-build -j
/tmp/jarvis-shell-build/jarvis-shell
```

Run requirements: a working Wayland or X11 display. The layer-shell
anchor only kicks in if `LayerShellQt` was found at configure time;
otherwise the shell falls back to a regular xdg-shell toplevel.

## Failure modes

| Failure | Behavior |
|---|---|
| Lilith service offline | LED red, bar accepts input but `Command` errors flow into the reply popup. |
| Permission daemon offline | `ApprovalDialog` never opens; dangerous-scope dispatches fail via the Action Bus's local fallback. |
| Updater daemon offline | `UpdaterSplash` never opens; if the model is missing, Lilith stays offline. |
| Voice daemon offline | MicButton renders disabled; tap is a no-op. |
| Settings daemon offline | SettingsBridge silently falls back to defaults; the panel's footer chip turns red. |
| Notifications daemon offline | Toasts + drawer go quiet; system.notify dispatches fail through the bus. |
| DBus session bus missing | Shell logs an error and runs with empty bridges — clock + launcher still work. |

## Out of scope (later phases)

- Multi-output layout polish (Phase 3, alongside the custom compositor).
- Glassmorphism shader pass (Phase 3 — needs our compositor for backdrop blur).
- Workspace switcher widget on the bar (waits on `workspace.*` actions
  becoming non-stubs, which waits on the Jarvis compositor).
