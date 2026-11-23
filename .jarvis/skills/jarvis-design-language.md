# Jarvis Design Language

## Visual Philosophy

LilithOS should feel:

- smooth and fluid
- futuristic but grounded
- elegant and minimal
- lightweight — never heavy
- alive without being distracting

The UI should look like it belongs in 2030, but remain practical for 8-hour work sessions.

## Reference

One reference, not a blend: the **Liquid Glass** language of macOS 26
(dark appearance) — ADR 0035. A fully transparent menu bar, a floating glass
dock, menus and panels as translucent material with a specular rim, one blue
accent, large soft radii, white labels at graded opacity.

We reproduce the *language* — layout, materials, proportions, motion. We do
not ship Apple's *assets*: no SF fonts (UI face is Inter), no Apple icons,
wallpapers, sounds, logo or names.

All values live in `shell/jarvis-shell/qml/Theme.qml`. A component never
spells out a colour, a radius or a duration — add a token instead.

The sci-fi command-centre HUD (ADR 0031) is an opt-in desktop *mode* with
its own palette, not part of this language.

## Motion Rules

- Animations must be <= 250ms
- Motion is modelled on Hyprland's defaults: surfaces **pop in** from 87 %
  scale on a strongly front-loaded curve (`Theme.curveOut`) and leave fast
  and linear (`Theme.animPopIn` / `animPopOut`). Use the tokens.
- Every menu, popover and dialog closes with Escape and with a click
  outside, and dialogs also have a visible ✕ — build them on
  `PopupSurface`, which gives all three.
- Prefer ease-out curves for all transitions
- Never animate more than one focal element simultaneously
- Blur and glass effects must never flicker or pop
- Spring animations are allowed for window open/close only
- Never use bouncy, cartoonish, or excessive motion

## Glassmorphism

### Allowed

- Taskbar background
- Notification panels and overlays
- Control center / quick settings
- Floating app launcher
- Context menus

### Avoid

- Heavy blur on text-heavy screens (documents, terminals, IDEs)
- Transparent backgrounds behind code editors
- Glass effects on performance-critical rendering paths
- Blur layers stacked more than 2 deep

### Implementation Rules

- Glass = blurred backdrop → tint → 1 px specular rim → soft shadow, and it
  is one component: `qml/GlassPanel.qml`. New surfaces start from it.
- The backdrop is the wallpaper, blurred ONCE (`WallpaperBackdropProvider`)
  and cropped to where the surface sits. Nothing is blurred per frame.
- A surface over application windows (or one the user can move) uses the
  opaque material: a wallpaper backdrop would be wrong there. Real blur of
  windows is a compositor feature (ADR 0034).
- Never wrap changing content in `layer.enabled` + an effect.

## Color & Depth

- Dark appearance; one accent (system blue). Semantic green / yellow / red only for state
- Use layered depth (z-levels) to communicate hierarchy
- Avoid flat design that removes all spatial cues
- Shadow systems should be soft, not harsh

## Typography

- One UI typeface: Inter. Monospace only for code and terminals
- Minimum readable body size: 13px
- Never sacrifice legibility for aesthetics
- Line-height >= 1.4 for all body text

## Accessibility

- All interactive elements must have keyboard equivalents
- Color must never be the sole way to communicate state
- Glass/blur must degrade gracefully when transparency effects are disabled
- Contrast ratio >= 4.5:1 for all text

## Non-Negotiable Rules

- UI must feel lightweight — never sluggish
- Readability always beats visual flair
- Every animation must have a purpose — not decoration
- Users working 8-hour sessions must not feel fatigued by the UI
