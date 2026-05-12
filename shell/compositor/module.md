# jarvis-compositor (placeholder)

## Status: PHASE 3 PLACEHOLDER

This crate is a skeleton kept in the workspace so the architecture is
visible at a glance, but **it is not built into the ISO** and does not
ship in any release. Production Jarvis OS images use [labwc][1] as the
Wayland compositor while this module gets fleshed out. See ADR 0006.

`iso/Containerfile`'s builder stage compiles `jarvis-action-bus`,
`jarvis-permission`, `jarvis-lilith`, `jarvis-updater` and skips this
crate explicitly with per-package `-p` flags rather than
`--workspace` — Smithay's transitive dependency on `libseat-sys`
otherwise pulls a long tail of Wayland build deps into the image we
don't want to pay for yet.

[1]: https://labwc.github.io/

## Purpose (eventual)

The Jarvis compositor will be the Wayland server: surfaces, window
lifecycle, input routing, GPU rendering, plus the bespoke window-arrange
behaviors Lilith needs to actually orchestrate the desktop ("tile
these three", "put this on the side display").

## Why a custom compositor at all

Three things we want that no off-the-shelf wlroots compositor gives us:

1. **First-class Action Bus integration** — window/workspace events are
   emitted directly onto the bus instead of being scraped via
   foreign-toplevel-management. Lilith reasons about windows the same
   way she reasons about anything else.
2. **Glassmorphism done right** — proper backdrop blur with the
   compositor controlling per-surface offscreen passes. Doable as a
   plugin to existing compositors, but the integration we want with
   the design language is closer to a fork than a plugin.
3. **AI-aware focus & input** — keyboard interactivity that's modal
   when Lilith is listening, transparent when she isn't. Hard to
   bolt on after the fact.

## Architecture target

```
jarvis-compositor      (this crate — Rust/Smithay)
      ↑ Wayland protocols
jarvis-shell           (Qt6/QML — bar, launcher, overlays via wlr-layer-shell)
other apps             (regular xdg-shell Wayland clients)
      ↕ DBus
jarvis-action-bus      (window.* and workspace.* registered here)
```

## Until then

- `window.*` and `workspace.*` Action Bus handlers return `UNAVAILABLE`.
- Lilith still has the tools listed in her catalog — when the
  compositor lands she gains those capabilities without a code change
  in Lilith.
- The shell's bar already speaks `wlr-layer-shell`, so swapping labwc
  out for `jarvis-compositor` is a session-config change, not a UI
  rewrite.

## Why keep the directory in the repo

A one-sentence justification for any abstraction is part of this
project's discipline (`.jarvis/skills/anti-bullshit-engineering.md`):
this directory exists because the *interface* between the rest of the
stack and a future compositor is part of Phase 1's architecture — the
Action Bus registry knows how to hand window handlers over to a
compositor at startup, the shell uses layer-shell expecting a
compositor that honors it, and labwc was chosen specifically because it
implements the same protocol set we'll target. Deleting the directory
would hide that intent.
