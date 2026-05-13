# jarvis-compositor (scaffold)

## Status: PHASE 4 SCAFFOLD (opt-in build)

The crate has been a workspace member from Phase 1 onward as an
architectural placeholder. Phase 4 promotes it to a real build
target: compiles in CI when `--build-arg BUILD_COMPOSITOR=1` is
passed to the Containerfile, ships at `/usr/bin/jarvis-compositor`,
and is available as an alternative greetd session for manual
testing.

Production ISOs still use [labwc][1] as the default compositor — the
Smithay scaffold doesn't yet render or accept input the way labwc
does, so flipping the default would regress every working desktop
feature. The point of the V1 scaffold isn't "replace labwc"; it's
"have a buildable, executable artifact at the right path so the
next phase of work isn't 'set up the toolchain again from scratch'."

Default builds skip it:

```bash
bash tools/build-iso.sh                  # labwc only, ~unchanged
```

To include the experimental compositor binary (also pulls libseat /
libinput / mesa-libgbm runtime libs into the final image, +~30 MB):

```bash
podman build --build-arg BUILD_COMPOSITOR=1 ...
```

When `BUILD_COMPOSITOR=0`, `/usr/bin/jarvis-compositor` is a stub
shell script that exits 127 with a clear message — better than a
missing path that breaks every `which jarvis-compositor` script.

## Switching the session to it (manual, for testing)

Edit `/etc/greetd/config.toml` post-install:

```toml
[default_session]
command = "/usr/libexec/jarvis-session-launch jarvis-compositor"
user    = "jarvis"
```

Restart greetd. Expect rough edges: no rendering on most setups
yet, input handling incomplete, no working window stack. The
scaffold builds and boots far enough to prove the architecture; it
doesn't yet replace labwc end-to-end. That's the multi-week arc
the scaffold exists to make tractable.

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
