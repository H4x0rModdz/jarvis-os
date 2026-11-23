# ADR 0034 — Compositor: stay on labwc, stop waiting for a custom one, shortlist for real blur

**Status:** accepted — its shortlist (decision 5) is superseded by ADR 0036 (Hyprland)
**Date:** 2026-11-23

## Context

P001 ("wlroots vs. from scratch vs. fork") has been open since Phase 1, and
since Phase 4 every missing desktop feature — window geometry, snapping,
workspaces, backdrop blur — has been filed as "waits for the Smithay
compositor". That compositor is a scaffold. Meanwhile the compositor is the
biggest graphics variable we have (ADR 0005 revalidation): it decides how well
NVIDIA works and whether glass can blur what is behind it.

The shell depends on two wlroots protocols: `zwlr_layer_shell_v1` (bar, dock,
every popup) and `zwlr_foreign_toplevel_manager_v1` (dock running state,
window control — ADR 0025). The image's tooling leans on more of the same
family: `grim` (screencopy), `wlr-randr` (output management), `wtype`,
`swayidle`.

### What was measured (2026-11-23, Fedora 44 packages, software rendering)

`jarvis-shell` was started under each candidate and its globals inspected.

| Compositor | In Fedora 44 | layer-shell | foreign-toplevel | Blur without client changes | Result |
|---|---|---|---|---|---|
| labwc 0.9.6 | main | yes | yes | no | shell correct (baseline) |
| sway 1.11 | main | yes | yes | no | shell correct; tiling WM |
| wayfire 0.10.1 | main | yes | yes | blur plugin — **needs GLES2**, refused on software | shell correct |
| niri 26.04 | updates | yes | yes | **yes — verified** (`layer-rule … background-effect { blur true }`) | shell correct; scrolling-tiling WM |
| KWin 6.8 | updates | yes | **no** (`org_kde_plasma_window_management` instead) | via `ext_background_effect` (client call) | would not composite without a GPU |
| Hyprland 0.57 | personal COPR only | — | — | by layer rule (untested) | would not start without a GPU |
| SwayFX 0.6 | COPR | — | — | `layer_effects` (untested) | would not start without a GPU |

Two findings apply to every compositor:

- **The shell's layer namespaces were all `window`.** `main.cpp` set the scope
  after the surfaces existed and LayerShellQt refused it, so no compositor
  rule could ever match the dock or the bar. Fixed with this ADR: every shell
  window declares scope, layer and anchors in QML before it is shown.
- **Unanchored shell popups were stretched over the whole screen**
  (LayerShellQt defaults to all four anchors). Also fixed.

The dev machine has no GPU, so nothing here says how any of them behaves on
NVIDIA, and three candidates could not be run at all.

## Decision

1. **Stay on labwc for now.** It is in Fedora main, stacking (the macOS-like
   model we want), small, and the shell is correct on it.
2. **Stop treating a custom Smithay compositor as the path to daily-driver
   features.** It stays as research. Snapping, workspaces and the `window.*` /
   `workspace.*` actions are to be delivered through the compositor we ship,
   not deferred (see the daily-driver plan).
3. **Glass does not wait for the compositor.** The shell draws a pre-blurred
   wallpaper behind its glass surfaces (`GlassPanel`, ADR 0035). That is real
   frosted glass over the desktop on any compositor; it does not blur windows.
4. **Keep the shell compositor-portable within the wlroots protocol family**
   — no labwc-only assumptions — so switching is a session change, not a port.
5. **Shortlist for real backdrop blur, to be decided on the RTX 3060:**
   - **wayfire** — stacking, same wlr protocols as today (nothing else in the
     image changes), blur plugin. Unverified here because it needs a GPU.
   - **niri** — blur verified, same protocols; but a scrolling-tiling WM, so
     only viable if we want that model or float every window.
   - **KWin** — the strongest NVIDIA story and real blur, at the price of
     porting the dock/window control to KDE's window-management protocol and
     replacing `grim`, `wlr-randr`, `wtype` and the lock path. Only if the
     wlroots options fail on NVIDIA.

   Hyprland and SwayFX are not candidates while they are COPR-only on Fedora.

## Consequences

- P001 is no longer "choose how to build a compositor" but "does wayfire (or
  niri) hold up on NVIDIA better than labwc" — an afternoon on the real
  machine, selectable from `/etc/jarvis/session.env` once a second session is
  packaged (next step, not in this change).
- Window-management work can start now, against labwc.
- Blur of *windows* behind panels (a menu over a browser) is still missing;
  those surfaces use the opaque material until the compositor provides it.

## Related

- ADR 0006 (Rust/Smithay compositor — now research), ADR 0024/0025 (window
  management on labwc), ADR 0035 (design language v2)
- `.jarvis/contexts/active-problems.md` P001, P005
