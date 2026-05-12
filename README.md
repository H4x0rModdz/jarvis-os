# Jarvis OS

> An AI-native desktop operating system. Lilith lives in the bar at the bottom of every screen, system actions flow through a single typed orchestration layer, and "open the browser to my dashboard" is a primitive — not a fragile script.

![status](https://img.shields.io/badge/status-Phase%202-blueviolet) ![base](https://img.shields.io/badge/base-Fedora%20bootc%2042-294172) ![shell](https://img.shields.io/badge/shell-Qt%206.10%20%2F%20QML-41cd52) ![ai](https://img.shields.io/badge/ai-Ollama%20%2B%20qwen3%3A4b-7c5cff)

---

## What this is

Jarvis OS treats AI as a **native operating-system component**, not a chatbot bolted on top of Linux. Everything the AI can do, you can do — and vice-versa — because both sides go through the same typed API.

- **Lilith** — local AI assistant (Ollama + `qwen3:4b` by default), always present in the bar, never speaks to the system except through actions.
- **Action Bus** — single DBus service every effect flows through. 28 actions across `app.*`, `file.*`, `window.*`, `browser.*`, `clipboard.*`, `screenshot.*`, `audio.*`, `system.*`. Each call is permission-checked, dispatched to a real handler, and audit-logged.
- **Permission System** — OS-level approval dialogs for dangerous scopes. `clipboard.read` and `screen.read` prompt the user the first time Lilith asks; the user can approve once or always.
- **Updater** — first-boot "Windows-Update style" splash that pulls Lilith's model from Ollama so the ISO stays slim (no 2.5 GB of weights baked in).
- **Shell** — Qt 6 / QML, bar anchored to the bottom of every output via `wlr-layer-shell`. No tray icon you have to click — the assistant input is right there.

## What this is not

These are explicit non-goals listed in [`.jarvis/jarvis-core-context.md`](./.jarvis/jarvis-core-context.md):

- Not a Windows clone.
- Not another Linux skin.
- Not a telemetry-driven surveillance platform.
- Not enterprise architecture theater in a desktop shell.
- **Not a chatbot that happens to have a desktop.**
- Not a research project that never ships.

If the answer to "what stops someone from doing this in an afternoon with `dnf install gnome-shell-extension-blur-my-shell && flatpak install Alpaca`?" is "nothing", that's the wrong design — and we've actively rejected it.

## Architecture

```
                    ┌──────────────────────────────────┐
                    │      jarvis-shell (Qt6/QML)      │
                    │   bar · launcher · approvals     │
                    │       · updater splash           │
                    └────┬──────┬───────────┬──────────┘
                         │      │           │
                  DBus   │      │           │
                         │      │           │
              ┌──────────┘      │           └───────────┐
              ▼                 ▼                       ▼
   ┌─────────────────┐  ┌───────────────┐  ┌─────────────────────┐
   │   Lilith        │  │ Permission    │  │     Updater         │
   │  (jarvis-       │  │  System       │  │ (jarvis-updater)    │
   │   lilith)       │  │ (jarvis-      │  │  pulls Ollama       │
   │ Ollama + tools  │  │  permission)  │  │  model, splash UI   │
   │ intent parser   │  │ scope policy  │  │  via DBus signals   │
   │ SQLite memory   │  │ + approvals   │  │                     │
   └────────┬────────┘  └───────┬───────┘  └──────────┬──────────┘
            │ Dispatch          │ Check               │ progress
            ▼                   │                     │
   ┌─────────────────────────────────────────────────────────────┐
   │                  Jarvis Action Bus                           │
   │            com.jarvis.ActionBus.Dispatch                     │
   │   28 actions · permission-gated · audit-logged               │
   └─────────────────────────────────────────────────────────────┘
            │
            ▼
   handlers/ (xdg-open · pkill · gio trash · wl-copy · grim · pactl …)
```

The compositor is currently **labwc** (production, ships in the ISO); a custom Smithay-based `jarvis-compositor` is parked as a Phase 3 placeholder in [`shell/compositor/`](./shell/compositor/module.md).

## Status

Phase 1 (foundation) is closed. Phase 2 (user-facing system) is in flight.

| Phase | What |
|---|---|
| **Phase 1 ✅** | Action Bus, Permission System, Lilith daemon, Qt shell with layer-shell bar, launcher, approval dialog, bootable Fedora-bootc ISO, first-boot updater. |
| **Phase 2 🚧** | Voice pipeline (Whisper STT + Piper TTS), Action Bus namespace expansion, OS-update flow on top of the updater, launcher focus restoration. |
| **Phase 3 ⏳** | Custom Smithay compositor (replaces labwc), Wine/Proton integration, Jarvis SDK for third-party action registration, custom greeter, glassmorphism shader pass. |

Current ISO: ~4.3 GB, boots in VirtualBox, autologin → labwc → bar at the bottom, splash → model pull → Lilith answers.

## Try it

The simplest path is to grab a build artifact from a GitHub Actions run on the [`main` branch](https://github.com/H4x0rModdz/jarvis-os/actions/workflows/build-iso.yml) and boot it in a VM (VirtualBox, qemu, libvirt — anything that boots a UEFI ISO).

```text
# In your VM:
# - 4 GB RAM, 25 GB disk, UEFI firmware
# - boot from jarvis-os-<version>.iso
# - Anaconda installer; pick the disk; Begin Installation
# - reboot, autologin as `jarvis`, watch the updater splash
# - bar appears, type something into the Lilith input
```

Anything else you'd normally do on a Fedora Atomic system works too (it *is* one).

## Build

End-to-end ISO build via Containerfile + bootc-image-builder:

```bash
# Inside the repo root
bash tools/build-iso.sh
# Output: iso/output/bootiso/install.iso
```

Requirements: podman 4.5+, ~10 GB free disk, internet access for `fedora-bootc:42` + Rust crates + Ollama installer.

Per-crate development on Linux (WSL2 works):

```bash
cargo test --workspace --exclude jarvis-compositor
# All daemons compile + their unit tests pass.

cmake -S shell/jarvis-shell -B /tmp/shell-build
cmake --build /tmp/shell-build -j
# Qt 6.5+ required — Theme singleton breaks under the Qt 6.4 compat
# mode. On Ubuntu 24.04 install Qt 6.8 via aqtinstall; see
# tools/dev/run-shell-labwc.sh.
```

## Repository layout

```
.
├── ai/lilith/             # AI assistant daemon (Rust)
├── system/
│   ├── action-bus/        # central orchestration DBus daemon (Rust)
│   ├── permission/        # scope policy + approval flow (Rust)
│   └── updater/           # first-boot asset puller (Rust)
├── shell/
│   ├── jarvis-shell/      # bar / launcher / dialogs (Qt 6 / QML)
│   └── compositor/        # Phase 3 placeholder (see its module.md)
├── iso/                   # Containerfile + assets + build.toml
├── tools/                 # build-iso.sh and a handful of dev helpers
└── .jarvis/               # project bible, ADRs, module contracts,
                           # skills, design language, current goals
```

Every module has a `module.md` documenting its boundaries, interface, and failure modes — read those before the source. The `.jarvis/` tree is the canonical context for both human contributors and AI agents.

## License

Open source, license TBD. Treat it as "look, learn, fork, don't ship as your own product" until a real license file lands.

## Pointers

- Architecture and rationale: [`.jarvis/jarvis-core-context.md`](./.jarvis/jarvis-core-context.md)
- Decisions log: [`.jarvis/decisions/`](./.jarvis/decisions/)
- Current goals: [`.jarvis/contexts/current-goals.md`](./.jarvis/contexts/current-goals.md)
- Engineering rules (no `utils.rs`, no `helpers/`, etc.): [`.jarvis/skills/`](./.jarvis/skills/)
