# 0022 — macOS-style shell layout: top menu bar + floating dock + Lilith orb

Status: accepted
Date: 2026-05-29

## Context

The shell shipped V1 as a single bottom bar (`Bar.qml`): a full-width
strip holding the launcher hamburger, clock, mic, an always-visible
Lilith text input, a status LED, and the wifi/bluetooth/battery/bell/
gear buttons. It works, but it reads like a phone status bar, not a
desktop. The user wants the familiar macOS arrangement: a thin **menu
bar** pinned to the top (system menu on the left, indicators + clock on
the right) and a **floating dock** centered at the bottom holding app
icons.

The hard part is Lilith. She is conversational, not an app you
"launch", so she has no obvious dock tile. We considered (a) a
persistent Spotlight-style input pill above the dock, (b) a separate
⌘Space command palette, and (c) an orb in the dock. The user chose the
orb.

## Decision

Replace the single bottom bar with **three wlr-layer-shell surfaces**,
all rendered by the one `jarvis-shell` process (three root objects in
one `QQmlApplicationEngine`; `main.cpp` configures each by
`objectName`):

| Surface | objectName | Layer | Anchors | Exclusive zone |
|---|---|---|---|---|
| Top menu bar (`Main.qml`) | `jarvis-topbar` | Top | top+left+right | height |
| Dock (`Dock.qml`) | `jarvis-dock` | Top | bottom (centered) | 0 (floats) |
| Desktop icons (`Desktop.qml`) | `jarvis-desktop` | Bottom | all four | 0 |

- **Top bar** — left: the Jarvis menu (an Apple-menu analogue: Sobre,
  Configurações, Atualização, Bloquear, Suspender, Reiniciar,
  Desligar). Right: the existing `BarWifiButton` / `BarBluetoothButton`
  / `BarBatteryIndicator` / `BarBellButton` / `Clock`, reused verbatim.
- **Dock** — a glass pill of pinned app tiles (Launchpad, Firefox,
  Dolphin, Zed, Terminal) plus, after a divider, the **Lilith orb**.
- **Lilith orb** — folds in the old status LED, mic button, and the
  popup trigger. Its glyph encodes state: `◉` idle, `◎` listening,
  `◌◌◌` thinking/processing, `◉◉◉` speaking. Click toggles the
  conversation popup; press-and-hold engages push-to-talk
  (`VoiceBridge`).

### Supporting changes

- **`ShellBus` singleton** — the dock and the top bar are separate root
  windows, so they can't reference each other's QML objects directly.
  A `pragma Singleton` `ShellBus` (shared across the engine) carries the
  cross-surface UI intents: `toggleLilith()`, `openLauncher()`,
  `openSettings()`, `openNotifications()`. The dock emits; `Main.qml`
  (which still owns every popup) listens.
- **`LilithPopup` gains a text input.** The always-visible bar input is
  gone, so the popup — now the only place to type to Lilith — grows a
  "Pergunte à Lilith…" field at its bottom and repositions above the
  dock instead of above the old bar.
- **`system.power` Action Bus action** (`{op: poweroff|reboot|suspend|
  lock}`) backs the Jarvis menu's power items. The shell can't shell
  out itself (Action Bus boundary), so the menu dispatches here.
  Scope `system.power` is deliberately *not* on the safe list.

## Consequences

- The bottom bar (`Bar.qml`) and its always-on input (`LilithInput`'s
  bar placement) are retired. The bar's child components that are still
  useful (Clock, indicator buttons, MicButton logic) are reused inside
  the new surfaces.
- Maximized windows sit *under* the floating dock (exclusive zone 0),
  exactly like macOS. Auto-hide is a future refinement, not V1.
- Three surfaces cost a little more compositor bookkeeping, but it's the
  same single process — no new systemd unit, no new daemon.
- Under the VMware software (pixman) renderer the extra glass surfaces
  add fill cost; the dock and orb keep their effects light, and the
  separate lag pass (ADR-pending) still applies.

## Alternatives rejected

- **Keep one bar, restyle it** — can't express the top-bar/bottom-dock
  split the user explicitly asked for.
- **Spotlight pill for Lilith** — always-visible input is not macOS and
  re-introduces the phone-status-bar feel we're removing.
- **A second process for the dock** — needless; one engine renders
  multiple layer-shell roots fine and shares the bridges + ShellBus.
