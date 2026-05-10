# Jarvis Design Language

## Visual Philosophy

Jarvis OS should feel:

- smooth and fluid
- futuristic but grounded
- elegant and minimal
- lightweight — never heavy
- alive without being distracting

The UI should look like it belongs in 2030, but remain practical for 8-hour work sessions.

## Inspirations

- Windows 11 (familiarity, taskbar ergonomics)
- macOS (polish, animation quality, hierarchy)
- KDE Plasma (customization, Linux-native feel)
- Sci-fi HUD systems (futuristic depth and glow)

## Motion Rules

- Animations must be <= 250ms
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

- Blur radius: 8–20px, never more
- Background opacity: 0.6–0.85, never fully transparent
- Always test blur on low-end GPU before shipping
- Glass should be optional/fallback on systems without GPU acceleration

## Color & Depth

- Prefer a dark base with subtle luminous accents
- Use layered depth (z-levels) to communicate hierarchy
- Avoid flat design that removes all spatial cues
- Shadow systems should be soft, not harsh

## Typography

- Prefer system fonts or a single, clean sans-serif
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
