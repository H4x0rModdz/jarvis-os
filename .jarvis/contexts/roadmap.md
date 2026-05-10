# Roadmap

## Stage 1 — Architecture & Foundation (Current)

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

## Principles Governing Prioritization

- Never move to the next stage without the current stage being solid
- A working simple system beats a broken complex one
- Documentation and tests are not optional — they gate stage completion
