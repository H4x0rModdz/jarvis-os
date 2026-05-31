# 0024 — macOS-style window management on labwc (window.* deferred to Smithay)

Status: accepted
Date: 2026-05-31

## Context

The shell ships a macOS-style top menu bar + a floating dock (ADR 0022),
but actual *window management* was never built. On the running labwc
session:

- **Window controls don't work.** `rc.xml` is minimal — it sets the
  `Default` theme and no titlebar/decoration rules. `foot` draws no
  titlebar of its own (CSD-none), so it has no close/min/max buttons at
  all. Dolphin gets labwc's server-side titlebar, but minimize sends the
  window nowhere the user can get it back from.
- **The dock is a static launcher.** It shows a fixed list of pinned
  apps + the Lilith orb. It is not a taskbar: running apps don't appear,
  there's no running indicator, minimized windows can't be restored from
  it, and tiles can't be reordered.
- **`window.*` / `workspace.*` Action Bus actions return `UNAVAILABLE`**
  (see `shell/compositor/module.md`) — by design, until the Jarvis
  Smithay compositor can register real handlers. That compositor is
  still a scaffold (a multi-week arc).

The user wants the desktop to feel like macOS: working traffic-light
controls, a dock that reflects running apps, drag-to-reorder, and
draggable windows. Waiting for the Smithay compositor to deliver any of
this is not acceptable.

## Decision

Build macOS-style window management **on the current labwc session now**,
using labwc's own facilities + the standard `wlr-foreign-toplevel-
management-v1` protocol, and keep the AI-native `window.*` Action Bus
surface deferred to the Smithay compositor.

Phased:

- **A — Decorations + controls (labwc config).** Configure `rc.xml`:
  titlebar button layout with close/minimize/maximize on the LEFT
  (macOS order), force server-side decorations for clients that ship
  none (foot) so every window has working buttons, rounded corners, and
  titlebar-drag / Alt-drag move + resize. The shell's own frameless
  layer-shell + popup surfaces are left untouched.
- **B — Dock as a taskbar (shell, foreign-toplevel).** A Qt bridge
  subscribes to `zwlr_foreign_toplevel_management_v1` (labwc implements
  it) and exposes a model of running toplevels (title, app_id,
  minimized/activated). `Dock.qml` shows running apps with a running
  indicator, click-to-focus, and click-to-restore for minimized windows.
  This is what makes minimize usable.
- **C — Reorder pinned tiles (shell).** Drag pinned dock tiles to
  reorder; persist the order to `com.jarvis.Settings`.
- **D — Move/resize/snap.** Titlebar-drag move + Alt-drag come from
  Phase A. Snapping and the full AI-callable `window.*` surface remain a
  Smithay concern.

## Why not wait for the Smithay compositor

The project already chose labwc as the placeholder *because it
implements the same protocol set we'll target* (ADR 0006 amendment,
`shell/compositor/module.md`). The foreign-toplevel bridge and the dock
UI built against it carry over to the Jarvis compositor unchanged once it
implements the protocol; the labwc `rc.xml` config is cheap and
throwaway. None of this work blocks or duplicates the Smithay arc — it
unblocks the user today.

## Consequences

- Window controls + move work on labwc without the custom compositor.
- The dock becomes a real taskbar; minimize stops "losing" windows.
- `window.*` Action Bus actions still report `UNAVAILABLE` — Lilith
  can't yet move/tile windows by voice. That capability lands with the
  Smithay compositor, at which point the dock bridge keeps working
  against the new server.
- labwc `rc.xml` is session config under `/etc` (OTA-deliverable). A
  malformed `rc.xml` degrades to labwc's built-in defaults rather than
  failing the session, but is still worth validating on a VM.
- Pixel-perfect macOS chrome (round traffic-light glyphs, genie
  minimize) is out of scope for the labwc theme; that level of polish
  waits on our own compositor's render pipeline.

## Alternatives rejected

- **Wait for the Smithay compositor for everything** — correct end state
  but multi-week; leaves the desktop with broken window controls in the
  meantime.
- **Force server-side decorations globally (`identifier="*"`)** — would
  also draw titlebars on the shell's own frameless popup windows
  (Launcher, dialogs, the Jarvis menu), breaking their design. We target
  the specific CSD-less clients (foot) instead.
