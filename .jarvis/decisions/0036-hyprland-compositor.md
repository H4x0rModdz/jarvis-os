# ADR 0036 — Hyprland is the session compositor; labwc is the fallback

**Status:** accepted — implemented, **not yet run on hardware**
**Date:** 2026-11-23
**Supersedes:** the shortlist in ADR 0034 (wayfire → niri → KWin). ADR 0034's
other decisions stand: no custom compositor on the path to features, and the
shell stays portable across wlroots-protocol compositors.

## Context

ADR 0034 kept labwc and left real backdrop blur to a later GPU trial. Two
things changed the same day:

- The first boot on real hardware made the gap concrete: labwc has no window
  animations, no blur, no rounded-corner clipping — the things the design
  direction (ADR 0035) is about.
- The project owner chose Hyprland directly, knowing it had been left off the
  shortlist only for packaging reasons.

Facts checked before committing to it:

- **Packaging.** Not in Fedora 44. Available from the COPR `sdegler/hyprland`
  (Hyprland 0.57.2, `hyprland-plugin-hyprbars`, `xdg-desktop-portal-hyprland`).
  `solopasha/hyprland`, the better-known COPR, has no Fedora 44 build.
- **Configuration is Lua** from 0.57 on (`hyprland.lua`); the old `.conf`
  format was dropped. Written against the example config and API stubs
  shipped in the package, not against older documentation.
- **Protocols.** Hyprland implements `wlr-layer-shell`,
  `wlr-foreign-toplevel-management`, `wlr-output-management`,
  `wlr-screencopy` and virtual keyboard — everything the shell and the
  Action Bus handlers (`grim`, `wlr-randr`, `wtype`, `swayidle`) use. Not
  verified here (it could not be started); taken from Hyprland's
  documentation.
- **It could not be run on the dev machine.** Hyprland needs a GPU; WSL has
  none, and a virtual DRM device (`vgem`) refuses the buffer allocation it
  needs. So nothing in this ADR has been seen working.

## Decision

1. **Hyprland is the default compositor** on machines with a GPU.
2. **labwc stays installed as the fallback**, chosen automatically by
   `/usr/libexec/jarvis-session-compositor` when
   - the host was put on software rendering (`WLR_RENDERER=pixman` from
     `jarvis-session-launch`: VMware, VirtualBox, no DRM render node), or
   - Hyprland exits with an error within 20 s of starting.
   It can also be chosen by hand: `JARVIS_COMPOSITOR=labwc` in
   `/etc/jarvis/session.env`, or the word `labwc` in
   `~/.config/jarvis/compositor`.
3. **One session, two compositors.** Both run
   `/usr/libexec/jarvis-session-autostart` (wallpaper, automount, daemons, the
   shell); the shell does not know which one it is on, except for one flag —
4. **Glass.** On Hyprland the shell's panels are tint-only and the compositor
   blurs what is really behind them, windows included (layer rules matched on
   the `jarvis-*` namespaces, `ignore_alpha` so transparent areas stay clear).
   On labwc they keep the pre-blurred-wallpaper backdrop (ADR 0035).
5. **Desktop model: floating.** Every window floats and stacks, as on macOS;
   tiling is a key (`Super+T`), not the default.
6. **Title bars** come from the `hyprbars` plugin (Hyprland draws none), with
   traffic-light buttons. The yellow one sends the window to a `minimized`
   special workspace (`Super+M` shows it) — Hyprland has no minimize.
7. **The config is fault-isolated.** `hyprland.lua` runs every block and every
   binding through `pcall` and logs failures to
   `~/.cache/jarvis-hyprland-config.log`, because it was written unseen.

## Consequences

**Gained:** window open/close/workspace animations, real blur, rounded
windows with shadows, workspaces and floating-window snapping, touchpad
gestures — without writing a compositor.

**Accepted costs and risks**

- **A third-party, one-maintainer package source** for the most critical
  process in the session. If the COPR lags a Fedora release, breaks, or
  disappears, the image falls back to labwc (and loses everything above)
  until we build Hyprland ourselves. The COPR is disabled after install so
  nothing else comes from it.
- **Hyprland moves fast and breaks configuration** (0.57 dropped the old
  format outright). Every Hyprland bump needs the session re-tested.
- **`hyprbars` must match the running Hyprland build exactly.** The COPR's
  plugin is versioned 0.57.0 against Hyprland 0.57.2; if it refuses to load,
  windows without client-side decorations (terminal, most Qt apps) have no
  title bar. Hyprland still runs, so the labwc fallback does NOT trigger.
- **No minimize**, so the dock's "minimized" state and the `window.minimize`
  action need rethinking on Hyprland.
- **Two compositors to keep working.** labwc's `rc.xml` and Hyprland's
  `hyprland.lua` duplicate key bindings and window policy.
- **NVIDIA:** Hyprland is not known to be better than wlroots on NVIDIA;
  0.57.1 shipped a fix for NVIDIA black screens. Unknown until tried.

## To verify on the machine, in this order

1. The session starts on Hyprland (`echo $XDG_CURRENT_DESKTOP` → `Hyprland`)
   and the menu bar and dock are in place.
2. `~/.cache/jarvis-hyprland-config.log` is empty.
3. Windows have title bars with three coloured buttons (hyprbars loaded).
4. The dock and menus are frosted over a window, not just over the wallpaper.
5. Dock: running dots, click-to-focus, close. Screenshot, lock, voice key.
6. If any of 1–3 fails: `echo labwc > ~/.config/jarvis/compositor`, log out.

## Related

- `iso/assets/hypr/hyprland.lua`, `iso/assets/launchers/jarvis-session-compositor`,
  `iso/assets/launchers/jarvis-session-autostart`, `iso/Containerfile.runtime`
- ADR 0034 (compositor trial), ADR 0035 (design language), ADR 0024/0025
  (window management through the shell)
