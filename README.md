# Jarvis OS

> An AI-native desktop operating system. Lilith lives in the bar at the bottom of every screen, system actions flow through a single typed orchestration layer, and "open the browser to my dashboard" is a primitive — not a fragile script.

![status](https://img.shields.io/badge/status-Phase%203-blueviolet) ![base](https://img.shields.io/badge/base-Fedora%20bootc%2042-294172) ![shell](https://img.shields.io/badge/shell-Qt%206.10%20%2F%20QML-41cd52) ![ai](https://img.shields.io/badge/ai-Ollama%20%2B%20qwen3%3A1.7b-7c5cff)

---

## What this is

Jarvis OS treats AI as a **native operating-system component**, not a chatbot bolted on top of Linux. Everything the AI can do, you can do — and vice-versa — because both sides go through the same typed API.

- **Lilith** — local AI assistant (Ollama + `qwen3:1.7b` by default), always present in the bar, never speaks to the system except through actions.
- **Action Bus** — single DBus service every effect flows through. 36 built-in actions across `app.*`, `file.*`, `window.*`, `browser.*`, `clipboard.*`, `screenshot.*`, `audio.*`, `system.*`, `updater.*`, `compat.*`, plus any action registered by an SDK app at startup. Each call is permission-checked, dispatched to a real handler, and audit-logged.
- **Permission System** — OS-level approval dialogs for dangerous scopes. `clipboard.read` and `screen.read` prompt the user the first time Lilith asks; the user can approve once or always.
- **Updater** — first-boot "Windows-Update style" splash that pulls Lilith's model from Ollama so the ISO stays slim (no 2.5 GB of weights baked in). OS upgrades via `bootc` plug in on the same surface.
- **Voice** — Whisper.cpp STT + Piper TTS, both local, running as a per-user daemon under a cpal actor (audio device is `!Send`). Push-to-talk in the bar; hotword later.
- **Notifications** — owns `org.freedesktop.Notifications`, so every `notify-send` lands on a Jarvis-style toast. Honours action buttons; drawer in the shell holds recent history.
- **Compat** — `com.jarvis.Compat` runs `.exe`s through Wine in per-app prefixes under `~/.jarvis/wine/`. The shared `default` prefix covers small tools; heavyweight apps each get their own.
- **Greeter** — custom greetd UI with a three-mode SwipeView (Standard / Lilith / Focus). Visually continuous with the lock screen and the shell.
- **Lock** — `com.jarvis.Lock` overlay via wlr-layer-shell; PAM auth through `pamtester`; `Super+L` everywhere.
- **Shell** — Qt 6 / QML, bar anchored to the bottom of every output via `wlr-layer-shell`. Launcher, approvals, notification drawer, settings panel, updater splash all live here.

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
                    │  notification drawer · settings  │
                    │     · updater splash             │
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
   │ Ollama + tools  │  │  permission)  │  │  model, OS bootc    │
   │ intent parser   │  │ scope policy  │  │  upgrades, splash   │
   │ SQLite memory   │  │ + approvals   │  │                     │
   └────────┬────────┘  └───────┬───────┘  └──────────┬──────────┘
            │ Dispatch          │ Check               │ progress
            ▼                   │                     │
   ┌─────────────────────────────────────────────────────────────┐
   │                  Jarvis Action Bus                           │
   │            com.jarvis.ActionBus.Dispatch                     │
   │   36 actions + SDK · permission-gated · audit-logged         │
   └─────────────────────────────────────────────────────────────┘
            │
            ▼
   handlers/ — xdg-open · pkill · gio trash · wl-copy · grim · pactl ·
              flatpak · wine (com.jarvis.Compat) · notify (com.jarvis.Notifications) ·
              updater (com.jarvis.Updater) · settings (com.jarvis.Settings) ·
              lock (com.jarvis.Lock) · voice (com.jarvis.Voice)
```

Pre-session: **greetd** spawns [`shell/jarvis-greeter/`](./shell/jarvis-greeter/module.md) (Qt overlay under `cage`). Post-login: the **labwc** compositor brings up the shell, the per-user daemons (voice, notifications, compat, lock), and Lilith. The custom Smithay-based `jarvis-compositor` is parked as a Phase 4 placeholder in [`shell/compositor/`](./shell/compositor/module.md).

## Status

Phase 1 and Phase 2 are closed. Phase 3 (Wine, SDK, greeter, lock, notifications V2) is in flight.

| Phase | What |
|---|---|
| **Phase 1 ✅** | Action Bus, Permission System, Lilith daemon, Qt shell with layer-shell bar, launcher, approval dialog, bootable Fedora-bootc ISO, first-boot updater. |
| **Phase 2 ✅** | Voice pipeline (Whisper STT + Piper TTS via cpal actor), Action Bus namespace expansion to 28, OS-update flow on top of the updater, launcher focus restoration, settings panel. |
| **Phase 3 🚧** | Wine compat (V1 shared prefix + V2 per-app), Flatpak app install via Action Bus, Jarvis SDK + example apps, custom greeter (3-mode SwipeView), lock screen V1 (layer-shell + pamtester), notifications V2 (action buttons + drawer + history). |
| **Phase 4 ⏳** | Smithay-based custom compositor, Proton via the Steam runtime, glassmorphism shader pass, hotword voice activation, idle auto-lock, biometric PAM modules, anime-avatar pipeline for Lilith mode. |

Current ISO: ~4.3 GB, boots in VirtualBox, autologin → labwc → bar at the bottom, splash → model pull → Lilith answers. From the greeter you also reach `Super+L` (lock), `notify-send` (toasts in our style), and `compat.run_exe` (Wine).

## Try it

The simplest path is to grab a build artifact from a GitHub Actions run on the [`main` branch](https://github.com/H4x0rModdz/jarvis-os/actions/workflows/build-iso.yml) and boot it in a VM (VirtualBox, qemu, libvirt — anything that boots a UEFI ISO).

```text
# In your VM:
# - 4 GB RAM, 25 GB disk, UEFI firmware
# - boot from jarvis-os-<version>.iso
# - Anaconda installer; pick the disk; Begin Installation
# - reboot, the greeter appears (Standard / Lilith / Focus modes)
# - log in as `jarvis`, watch the updater splash
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

Requirements: podman 4.5+, ~10 GB free disk, internet access for `fedora-bootc:42` + Rust crates + Ollama installer + Flathub remote.

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
├── ai/lilith/                  # AI assistant daemon (Rust)
├── system/
│   ├── action-bus/             # central orchestration DBus daemon (Rust)
│   ├── permission/             # scope policy + approval flow (Rust)
│   ├── updater/                # first-boot asset puller + bootc upgrades (Rust)
│   ├── settings/               # com.jarvis.Settings, SQLite-backed (Rust)
│   ├── voice/                  # com.jarvis.Voice, Whisper + Piper + cpal (Rust)
│   ├── notifications/          # org.freedesktop.Notifications server (Rust)
│   ├── compat/                 # com.jarvis.Compat, Wine prefix runner (Rust)
│   └── lock/                   # com.jarvis.Lock, session lock daemon (Rust)
├── shell/
│   ├── jarvis-shell/           # bar / launcher / dialogs / drawer (Qt 6 / QML)
│   ├── jarvis-greeter/         # custom greetd UI, 3-mode SwipeView (Qt 6 / QML)
│   ├── jarvis-lock/            # full-screen lock overlay (Qt 6 / QML)
│   └── compositor/             # Phase 4 placeholder (see its module.md)
├── sdk/
│   ├── jarvis-sdk-types/       # action manifest schema (shared crate)
│   └── jarvis-sdk-rust/        # thin DBus helper for SDK app authors
├── examples/
│   └── jarvis-app-hello/       # minimal SDK app — registers one action
├── tools/
│   ├── build-iso.sh            # entry point
│   ├── lock-ctl/               # Super+L → com.jarvis.Lock.Lock()
│   └── dev/                    # run-shell-labwc.sh, ad-hoc helpers
├── iso/                        # Containerfile + assets + build.toml
└── .jarvis/                    # project bible, ADRs, module contracts,
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
