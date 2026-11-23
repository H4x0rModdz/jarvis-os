# jarvis-shell

## Purpose

The LilithOS desktop shell — a Qt6/QML application that paints the
transparent **menu bar**, the floating glass **dock** (app tiles +
the Lilith orb), an optional desktop HUD, the launcher, permission
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

### How the surfaces are placed

Every shell window is a `wlr-layer-shell` surface (`main.cpp` switches Qt's
Wayland integration to layer-shell before the app is created) and **declares
its own placement in QML** with the `org.kde.layershell` attached properties:
`LayerShell.Window.scope` (the namespace compositor rules match on), `.layer`,
`.anchors`, `.exclusionZone`, `.keyboardInteractivity`. A Wayland client
cannot position a window with x/y, so:

- menus, popovers and dialogs do not place a small window at all: they are a
  `PopupSurface` (a transparent surface covering the output, with the panel
  positioned inside it) — see Design system below;
- any other window anchors explicitly; one left to LayerShellQt's default
  (all four edges) is stretched fullscreen;
- the values must be set declaratively, before the window is first shown; a
  namespace set after the surface exists is silently refused.

### Design system

| File | Role |
|---|---|
| `qml/Theme.qml` | Singleton design tokens — colour, glass materials, radii, type, motion (ADR 0035). The only place a literal value may appear. |
| `qml/GlassPanel.qml` | The glass surface all chrome is made of: blurred wallpaper backdrop → tint → specular rim → shadow. `backdrop: true` + `windowX/Y` where the surface's screen position is known; otherwise the opaque material. |
| `qml/PopupSurface.qml` | Base of every menu, popover and dialog: a fullscreen transparent layer surface holding the panel, so Escape always closes it (exclusive keyboard), a click outside closes it, and it animates in/out. `present()` / `dismiss()` / `shown` — never assign `visible`. |
| `qml/SymbolIcon.qml` | A `-symbolic` status glyph from the icon theme, tinted to a colour, with a character fallback. |
| `src/wallpaper_backdrop_provider.*` | `image://backdrop/blur` — the wallpaper, blurred once at tiny resolution. |
| `src/icon_image_provider.*` | `image://theme/<name>` — freedesktop icons. |

### Menu bar

| File | Role |
|---|---|
| `qml/Main.qml` | Root layer surface (scope `jarvis-topbar`, anchored top, exclusive zone = bar height). Holds `TopBar` + every popup Window. Listens to `ShellBus` for intents from the dock. |
| `qml/TopBar.qml` | The transparent bar: system mark + focused app name (left); Lilith, battery, Bluetooth, network, search, Control Centre, date/time (right). Minute-resolution clock. |
| `qml/BarItem.qml` | One clickable bar item with the hover / open highlight. |
| `qml/JarvisMenu.qml` | The system menu under the mark: about, settings, update, sleep/restart/shut down/lock (the power rows dispatch `system.power`). |
| `qml/ShellBus.qml` | Singleton signal bus carrying cross-surface UI intents (dock → top-bar popups): `toggleLilith` / `openLauncher` / `openSettings` / `openNotifications`. |

### Dock

| File | Role |
|---|---|
| `qml/Dock.qml` | Floating glass dock (scope `jarvis-dock`), anchored bottom, centred by the compositor; exclusive zone = the glass bar, so maximized windows stop above it. Pinned apps + running apps + divider + the Lilith orb. App tiles dispatch `app.open`; the launcher tile and orb route through `ShellBus`. |
| `qml/DockIcon.qml` | One tile: theme icon (letter stand-in if the theme has none), magnifies upward on hover, name bubble, running dot, context menu. |
| `qml/LilithOrb.qml` | Lilith's dock presence — a luminous orb whose colour and breathing carry her state (idle / listening / thinking / speaking / offline). Click → `ShellBus.toggleLilith`; press-and-hold → push-to-talk. |

### Desktop

| File | Role |
|---|---|
| `qml/Desktop.qml` | The optional command-centre HUD (scope `jarvis-desktop`, bottom layer, all four edges). **Off by default** — the desktop is the wallpaper; turned on in Preferências (`desktop.hud`). |
| `qml/HudGauge.qml`, `qml/HudGraph.qml` | The HUD's dials and line graphs. |

### Popups and dialogs

| File | Placement | Role |
|---|---|---|
| `qml/Launcher.qml` | top-centre | Spotlight-style app search + grid, backed by `DesktopAppsModel`. |
| `qml/AppCell.qml` | — | One app tile in the launcher grid. |
| `qml/ConnectivityPanel.qml`, `qml/BluetoothPanel.qml`, `qml/NotificationDrawer.qml` | top-right | Wi-Fi, Bluetooth, notification history. |
| `qml/NotificationToast.qml` | top-right | Incoming notification, with action buttons; takes no focus. |
| `qml/LilithPopup.qml` | bottom-centre, above the dock | The Lilith conversation. |
| `qml/SettingsPanel.qml`, `qml/DisplayPanel.qml`, `qml/AboutDialog.qml` | centred | Preferences, displays, about. |
| `qml/ApprovalDialog.qml` (+ `ApprovalButton.qml`) | centred | Driven by `PermissionBridge.hasPending`. |
| `qml/UpdaterSplash.qml`, `qml/FirstBootWizard.qml` | centred | First-boot / OS-upgrade splash; first-run wizard. |
| `qml/LilithAvatar.qml`, `qml/LilithAvatarView.qml` | bottom-right | The 3D avatar (ADR 0028), only when a VRM is present. |

### Looking at it

`tools/dev/shell-preview.sh live` opens a window with the shell running in it;
`… shot <name>` writes `.preview/<name>.png`. Both build the shell with the
OS's Qt inside a container and run it under labwc with the session's `rc.xml`
and window theme. Dev switches read by `main.cpp`: `JARVIS_SKIP_FIRST_BOOT`,
`JARVIS_HUD`, `JARVIS_ICON_THEME`, `JARVIS_WALLPAPER`, `JARVIS_SHELL_SURFACES`,
`JARVIS_QML_PATH`.

### Bridges (C++ singletons exposed as `Jarvis.Shell.*`)

| Bridge | Talks to | What it does |
|---|---|---|
| `LilithBridge` | `com.jarvis.Lilith` | `Command(text)` + reachability (DBus name watcher, not a poll) + busy state. |
| `PermissionBridge` | `com.jarvis.PermissionSystem` | Subscribes to `ApprovalRequested`, exposes pending state, sends `ResolveApproval`. |
| `ActionBusBridge` | `com.jarvis.ActionBus` | Direct `Dispatch(json)` for shell-internal callers (launcher tile click). |
| `UpdaterBridge` | `com.jarvis.Updater` | Progress + Completed + OSUpdateAvailable signals, `applyOSUpgrade()`. |
| `VoiceBridge` | `com.jarvis.Voice` | `toggle()`, subscribes to StateChanged + TranscriptionFinal. |
| `SettingsBridge` | `com.jarvis.Settings` | Sync getters + async setters + Changed signal re-emit. |
| `NotificationsBridge` | `com.jarvis.Notifications` + `org.freedesktop.Notifications` | Re-emit NotificationPosted, `invokeAction(id, key)`, history list. |
| `DesktopAppsModel` | XDG `applications/` dirs | Scan + filter for the launcher. |
| `SystemStatsBridge` | `/proc`, `/sys` | HUD telemetry. Sampling runs on a worker thread (`SystemStatsSampler`); the bridge republishes with one change signal per property group, emitted only on a visible change. |
| `MediaBridge` | `playerctl --follow` | Now-playing line + transport controls for the HUD. Event-driven — one long-lived child process, nothing polled. |
| `AudioBridge` | `pactl` | Output-device list for Preferences. Polls only while the panel is open (`startPolling`/`stopPolling`). |
| `NetworkBridge` / `BluetoothBridge` | `nmcli` / `bluetoothctl` | Wi-Fi and Bluetooth panels. Poll only while their panel is open. |

## Performance rules

All three surfaces and every popup share **one GUI thread**. Anything that
stalls it freezes the bar, the dock and the HUD at once, so these are not
style preferences — each one is here because breaking it caused a visible
hitch on real hardware:

1. **Nothing blocks the GUI thread.** No `waitForFinished()`, no synchronous
   DBus `call()`, no directory walks. Subprocesses are async; file sampling
   that repeats belongs on a worker thread (see `SystemStatsSampler`).
2. **Events over polling.** If the source can tell us when it changed (a DBus
   signal, a name-owner change, a `--follow` stream), a timer is the wrong
   tool. Where polling is unavoidable, it runs only while its UI is visible.
3. **Notify only what changed.** A bridge with many properties gets one
   change signal per group and compares before emitting. One blanket
   `updated()` re-evaluates every binding that touches the bridge.
4. **Repeater models must be stable.** A list property that is replaced on
   every update destroys and recreates all its delegates. Drive the Repeater
   with a count and read values by index when only the values change.
5. **No `layer.enabled` + effect around changing content.** It re-renders
   the subtree to a texture and re-runs the effect on every change inside.
   `GlassPanel` draws its shadow as a separate item behind the body for
   this reason.
6. **The desktop surface is fullscreen** — one changed pixel re-renders and
   recomposites the whole output. Nothing on it animates at rest, and 1 Hz
   telemetry snaps rather than tweens.

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

Out-of-tree CMake. Qt 6.9+ required (`GlassPanel` uses `RectangularShadow`,
added in 6.9). Fedora 44 ships Qt 6.11, which is what the image build uses;
local dev on Ubuntu 24.04 needs Qt 6.9+ via aqtinstall, or build inside the
`jarvis-builder` container.

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
