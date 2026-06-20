# LilithOS — Core Context

> This is the project "bible". Read this first. It defines what LilithOS is and is not.

## What LilithOS Is

- An AI-native desktop operating system
- Linux-based (custom kernel is future research, not current goal)
- Open source
- Desktop-first
- Compatibility-oriented (Windows apps should feel native via Wine/Proton)
- Built for real people doing real daily work

## What LilithOS Is NOT

- A Windows clone
- Another Linux skin
- A telemetry-driven surveillance platform
- Enterprise architecture theater in a desktop shell
- A chatbot that happens to have a desktop
- A research project that never ships

## The Central Innovation

LilithOS treats AI as a **native operating system component**, not a bolt-on.

This changes:
- Architecture (Action Bus exists because AI needs structured system access)
- APIs (everything is action-based because AI needs structured schemas)
- UX (permissions dialogs exist at the system level, not per-app)
- Development (every module is designed to be readable by AI agents)

## Core Components

| Component | Purpose |
|---|---|
| Lilith | AI assistant daemon, native to the OS |
| Action Bus (JAB) | Single orchestration layer for all system interactions |
| Permission System | Enforces AI and app access controls |
| Compositor | Wayland compositor with glassmorphism effects |
| Window Manager | Window lifecycle, workspaces, tiling |
| Voice Pipeline | STT (Whisper) + TTS (Piper), fully local |
| Compatibility Layer | Wine + Proton for Windows app support |
| Jarvis SDK | Allows apps to expose actions and AI integration |

## Technology Decisions (Already Made)

- OS base: Linux — Fedora Atomic, OCI image model (ADR 0001, ADR 0005)
- UI framework: Qt6/QML in C++ for shell UI (ADR 0002, ADR 0006)
- Compositor: Rust via Smithay (ADR 0006)
- System daemons: Rust (Action Bus, Permission System, Lilith, Automation)
- Display protocol: Wayland-first (ADR 0003)
- System interactions: Centralized Action Bus via DBus (ADR 0004)
- Shell↔System boundary: DBus only — no FFI, no shared memory
- Distribution model: OCI image (BlueBuild), atomic updates, immutable base (ADR 0005)
- Implementation order: Action Bus → Compositor → Lilith
- Lilith default model: qwen3:4b via Ollama (local, low VRAM)

## Design Language Summary

- Glassmorphism: allowed, but subtle (blur 8-20px, opacity 0.6-0.85)
- Animations: <= 250ms, ease-out, purposeful
- Feel: smooth, futuristic, elegant, lightweight
- Inspirations: Windows 11 + macOS + KDE Plasma

## Engineering Philosophy Summary

- Anti-bullshit: no overengineering, no premature abstraction
- Context-oriented: every name explains itself, every module has `module.md`
- AI-readable: predictable structure, explicit contracts, semantic naming
- Modular: every subsystem is independently understandable

## What Good Looks Like

A new engineer or AI agent reads the repository and within 10 minutes can:
1. Understand what each top-level folder contains
2. Find the module that handles any given responsibility
3. Read a `module.md` and understand the module's purpose and API
4. Trace an action from user input to system execution

## Current Status

Phase 1 — Architecture & Foundation.
See `.jarvis/contexts/current-goals.md` for active work.
See `.jarvis/contexts/roadmap.md` for full roadmap.
