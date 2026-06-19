# Roadmap

> **Where we actually are:** Stages 1–6 are substantially built (core runtime,
> shell, AI, voice, compat, SDK all ship). We're in **Stage 7 — Public Alpha**:
> installable ISO + OTA exist; the active work is daily-driver hardening
> (see `current-goals.md` and ADRs 0025–0027) and the Smithay compositor
> (P001). The stage list below is the original coarse plan, kept for reference.

## Stage 1 — Architecture & Foundation

- System architecture documentation
- Repository structure and standards
- Module contracts for core subsystems
- CI/CD pipeline setup
- Development environment documentation

## Stage 2 — Core Runtime Prototype

- Action Bus implementation (DBus IPC)
- Basic permission system
- Lilith daemon skeleton
- Simple intent routing (rule-based, no LLM required)
- Basic window manager on Wayland (using wlroots)

## Stage 3 — Desktop Shell

- Compositor with basic GPU rendering
- Taskbar
- App launcher
- Notification system
- Basic settings panel

## Stage 4 — AI Integration

- Lilith LLM integration (Ollama local model)
- Voice pipeline (STT + TTS)
- AI memory system
- AI-callable action set

## Stage 5 — Compatibility Layer

- Wine/Proton runner integration
- Windows app launcher
- Compatibility metadata system
- Steam/gaming integration basics

## Stage 6 — Developer SDK

- Jarvis SDK API surface
- App action registration
- SDK documentation
- Example applications

## Stage 7 — Public Alpha

- Installable ISO or distribution package
- User documentation
- Bug reporting system
- Community channels open

## Daily-driver feature backlog (planned 2026-06-19)

User wishlist for "live in it daily", grouped to keep PRs reviewable and CI
sane. Sizes: **P** ≈ hours, **M** ≈ a day, **G** ≈ multi-day, **GG** ≈ weeks.
None of these are started; each epic gets its own ADR when its approach is
picked. Order below is the recommended sequence.

### PR 1 — "Lilith does more + daily apps" (one PR, several commits) — P/M
The quick, additive, low-risk batch — most daily value per effort.
- **Apps**: image viewer, PDF viewer, media player as Flatpaks in the runtime
  base (decision: which — e.g. Loupe / papers / Celluloid). [P]
- **`input.type`**: new Action Bus action so Lilith can type into the focused
  app ("escreve oi no zed"). Needs `wtype` (or ydotool) in the runtime base +
  a Lilith tool. [M]
- **Wallpaper change**: `desktop.set_wallpaper(path|url)` — set from a folder
  path or download from a URL, persist + re-apply via swaybg. [M]
- Optionally 1–2 more simple Lilith tools while we're in there.

### PR 2 — Lilith web search — M/G
- A `web.search` tool (+ fetch/summarise a page) so Lilith can answer from the
  internet — also unlocks "find a wallpaper online" on top of PR 1.
- **Decision needed**: search backend. Privacy-first without keys
  (DuckDuckGo HTML / a SearXNG instance) vs an API with a key (Brave, etc.).

### PR 3 — Qt/KDE theming (Kvantum) — M
- Make Dolphin + other Qt apps match WhiteSur via the WhiteSur Kvantum theme +
  qt6ct/env, including the Flatpak `org.kde.KStyle.Kvantum` extension.
- Known to be fiddly for Flatpak KDE apps — own PR so it can fail in isolation.

### Epics (each its own ADR + PR(s); do NOT cram into the batches above)
- **Lilith 3D body / avatar — GG (weeks).** Model tiers: *basic* (today's
  slot) and *realistic* — an always-on 3D body in the corner that idles
  quietly, wakes on click or hotword, talks with lip-sync, and shows emotions /
  facial expressions mapped from her replies. Stack: Qt Quick 3D + a rigged
  model (glTF/VRM) + animation/expression state + a performance tier toggle
  (realistic uses more GPU). Builds on the Phase 4 "anime avatar slot". Needs
  an ADR for the tech + asset pipeline before any production code.
- **Custom installer + user/password registration — G.** A real first-install
  flow that registers a username + password (drop the dev `jarvis/jarvis`
  default) and ties off LUKS at-rest encryption (ADR 0027). Anaconda kickstart
  customisation vs a custom installer UI is the open decision.
- **Multi-monitor + HiDPI (fractional scaling) depth — G.** `DisplayPanel`
  exists; making 2+ monitors + fractional scaling actually work needs real
  hardware to validate (the VM can't exercise it). Partly blocked on the
  Smithay compositor (P001) for full window/output control.

## Principles Governing Prioritization

- Never move to the next stage without the current stage being solid
- A working simple system beats a broken complex one
- Documentation and tests are not optional — they gate stage completion
