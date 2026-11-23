# ADR 0035 — Design language v2: one reference (Liquid Glass), tokens, real placement

**Status:** accepted
**Date:** 2026-11-23
**Supersedes:** the "Windows 11 + macOS + KDE + sci-fi HUD" blend in
`skills/jarvis-design-language.md`; the default-on desktop HUD of ADR 0031

## Context

The shell looked unfinished, and not only for reasons of taste. Looking at it
running (`tools/dev/shell-preview.sh`) showed structural causes:

- four inspirations and no single reference, so every panel was styled alone;
- status glyphs hand-drawn from rectangles, app icons falling back to letters;
- every dialog stretched over the whole screen, and every popover placed by
  the compositor rather than under the control that opened it (a Wayland
  client cannot position a window with x/y — see ADR 0034);
- a violet accent bloom on every card, an opaque grey bar, labwc's stock grey
  title bars;
- a dense cyan sci-fi HUD as the default desktop, opposite in every way to the
  chrome around it.

Direction from the project owner: the shell follows the design of macOS 26
"Tahoe" (Liquid Glass). Icons need not match.

## Decision

1. **One reference.** The design language is Liquid Glass, dark appearance:
   a fully transparent menu bar, a floating glass dock, menus and panels as
   translucent material with a specular rim, one blue accent, large soft
   radii, white labels at graded opacity, quick ease-out motion.
2. **We take the language, not the assets.** Layout, materials, proportions
   and motion are reproduced. Apple's fonts (the SF Pro licence limits it to
   mock-ups for Apple platforms), icons, wallpapers, sounds, logo and names
   are not shipped. UI typeface: **Inter**. Status glyphs: the icon theme's
   `-symbolic` set, tinted in the shell. Our own mark (a crescent) and names.
3. **Tokens.** `Theme.qml` is the single source for colour, shape, type and
   motion. No literal colour, radius or duration in a component.
4. **Glass is one component** (`GlassPanel`): blurred wallpaper backdrop →
   tint → specular rim → analytic shadow. The backdrop is pre-blurred once
   (`WallpaperBackdropProvider`); nothing is blurred per frame.
5. **Every shell window places itself** with layer-shell anchors declared in
   QML: popovers hang from their corner of the menu bar, the launcher sits
   top-centre, dialogs are centred at their own size, the Lilith conversation
   sits above the dock.
6. **Window decorations match:** a labwc theme (`iso/assets/labwc/theme`) with
   a dark title bar, centred title and traffic-light buttons.
7. **The command-centre HUD becomes a mode** (`desktop.hud`, off by default).
   The default desktop is the wallpaper.

## Consequences

- Qt 6.9+ is required (already the case since the performance work).
- Popovers over application windows use the opaque material: their backdrop
  would be the wallpaper, which is wrong over a window. Real blur there is a
  compositor feature (ADR 0034).
- The backdrop needs each glass surface to know where it is on screen; a
  surface the user can move freely cannot use it.
- A wallpaper change is picked up by the backdrop on the next shell start, not
  live.
- Light appearance is not implemented.

## Still on the old look (follow-up work — see the daily-driver plan)

Control Centre and Notification Centre as proper panels; the insides of
Preferências, Wi-Fi, Bluetooth, Displays, About, the approval dialog, the
updater splash and the first-boot wizard; the greeter and the lock screen;
GTK/Qt application theming; dock drag-to-reorder and true magnification.

## Verification

Built and run under labwc in the preview container: QML loads with no errors
or warnings; screenshots of the desktop, system menu, launcher, Preferências
and the Lilith conversation; idle shell CPU 0% of a core (1% with the HUD on),
software rendering at 2560x1080. Not yet seen on real hardware.
