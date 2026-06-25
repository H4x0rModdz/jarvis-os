# 0031 — eDEX-style desktop HUD (command center)

Status: accepted
Date: 2026-06-20

## Context

The user wants the home/desktop to look like [eDEX-UI](https://github.com/gitsquared/edex-ui)
— a sci-fi command center with live system telemetry (CPU/RAM/network graphs,
top processes, clock/uptime) in glowing monospace panels. Not as a separate
fullscreen app, but **on the home surface**, behind app windows (like conky /
desktop widgets).

Two tensions to resolve up front:

- **Design language.** The project's chrome is calm glassmorphism (purple
  accent, animations ≤250 ms). eDEX is the opposite — dense, always-animating,
  Tron cyan. A faithful HUD deliberately departs from the house style.
- **Real terminal.** eDEX's centerpiece is a full terminal (xterm.js). QML has
  no terminal widget; a real ANSI emulator is a large, separate effort.

## Decision

1. **HUD lives on the Desktop surface (`Desktop.qml`).** It's already a separate
   layer-shell *bottom* root (below app windows, no keyboard) — exactly the
   "ambient widget behind windows" placement the reference shows. Left **SYSTEM**
   panel + right **NETWORK** panel; desktop icons shift right to clear the left
   panel.

2. **"Command center" soul, not a generic eDEX clone.** The panels are
   Lilith's — system telemetry now, with the 3D avatar + Action-Bus activity
   feed as the planned center (reusing existing work). The differentiator is
   that it's *her* console, not just sysmon+terminal.

3. **Telemetry via a new `SystemStatsBridge` (C++, /proc).** Polls `/proc/stat`,
   `/proc/meminfo`, `/proc/net/dev`, `/proc/uptime` and `/proc/<pid>` once a
   second; exposes CPU (overall + per-core + history), memory, swap, network
   up/down (+ history), uptime, task count, CPU model, and top-5 processes by
   RSS as QML properties. Read-only, no subprocess, no privilege; non-Linux dev
   host yields zeros (empty-but-valid HUD), never a crash.

4. **The HUD is a deliberate sci-fi *mode* with its own cyan palette** —
   an explicit, scoped exception to the glassmorphic default (recorded here so
   it isn't "fixed" back to house style by mistake). It does not restyle the
   bar/dock/popups.

5. **Real terminal is deferred** (Phase 2). The first cut is telemetry +
   aesthetic + (next) the Lilith center. A real terminal means a PTY + ANSI
   emulator in QML, or embedding `foot` — its own effort.

## Consequences

- New `SystemStatsBridge` (shell C++); a reusable `HudGraph.qml` (Canvas line
  graph); `Desktop.qml` grows the two panels. No new daemon, no new dependency.
- Top processes are ranked by **memory (RSS)** in v1 — instantaneous from
  `/proc/<pid>/statm`, no per-process CPU-delta bookkeeping. Per-process CPU%
  is a v2 add behind the same property.
- The HUD renders behind app windows; maximized windows cover it (intended —
  it's a desktop widget, not a fullscreen overlay).
- Rendering can't be validated on the dev host (no GPU/preview) → on-device
  tuning on the VM, like the avatar. The C++/QML compile in the image pipeline.

## Alternatives rejected

- **Summonable fullscreen HUD overlay** — the user explicitly wants it on the
  home, behind windows.
- **Reskin the whole shell in Tron style** — too invasive; fights the glass
  chrome everywhere. The HUD is a contained desktop surface instead.
- **Ship a real terminal now** — high effort (ANSI emulator / Wayland embed);
  not needed for the first, telemetry-focused cut.

## Related

- `shell/jarvis-shell/qml/Desktop.qml`, `qml/HudGraph.qml`,
  `src/system_stats_bridge.{h,cpp}`.
- ADR 0028 (embodied avatar — the planned HUD center).
- `.jarvis/skills/jarvis-design-language.md` (the default this mode departs from).
