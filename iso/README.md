# Jarvis OS — ISO build

This directory turns the repo into a bootable Jarvis OS install ISO. It
follows the path locked in by [`.jarvis/decisions/0005-fedora-atomic-base.md`](../.jarvis/decisions/0005-fedora-atomic-base.md):
**Fedora Atomic (bootc) + labwc + our Rust/Qt stack baked into an
immutable OS image**.

## What's in v0.0.1

| Component | Notes |
|---|---|
| Base | `ghcr.io/ublue-os/wayblue-labwc:latest` (Fedora Atomic + labwc) |
| Compositor | labwc 0.7+ |
| Shell | `jarvis-shell` (Qt 6, layer-shell, glassmorphic bar) |
| Daemons | `jarvis-permission`, `jarvis-action-bus`, `jarvis-lilith` — user systemd services |
| AI runtime | Ollama installed, model **not** included — pulled on first boot via the updater |
| Default user | `jarvis` / `jarvis` (dev password, change after install) |

The ISO is intentionally slim — about 1.5 GB without the LLM. The
post-first-boot "Jarvis Updater" (still TODO, lives in a follow-up) will
download `qwen3:4b` (~2.5 GB) the first time you go online.

## Build locally

```bash
bash tools/build-iso.sh
```

Requires `podman` 4.5+ and roughly 10 GB of free disk. The script:

1. Builds the multi-stage `Containerfile` — Fedora 42 builder compiles
   every Rust crate and the Qt shell, then those binaries land on the
   final `wayblue-labwc` image.
2. Runs `bootc-image-builder` to turn the OCI image into a bootable ISO.

Output drops in `iso/output/bootiso/install.iso`.

## Boot it in VirtualBox

1. **Create the VM**
   - Type: Linux, Version: Fedora (64-bit)
   - Base memory: **4096 MB** minimum (the shell + Ollama want some headroom)
   - Processors: 2+ vCPUs
   - Virtual disk: **30 GB** (model + scratch space)
2. **Settings → System → Motherboard → check "Enable EFI"**
   bootc images boot via UEFI. Legacy BIOS won't find the loader.
3. **Settings → Display → Graphics Controller: VMSVGA**, Video Memory ≥ 128 MB.
4. **Settings → Storage → attach `install.iso` to the optical drive.**
5. **Start the VM.** Anaconda comes up. Pick the disk, accept the layout,
   set hostname, finish install. Reboot.
6. After reboot, log in as `jarvis` / `jarvis`. labwc starts and the bar
   appears at the bottom of the screen.

## What works on first boot

- The bar (clock, input, status LED — green)
- Lilith via the regex rule path (`abrir firefox`, `notify: oi`, etc.)
- Action Bus dispatch
- Permission system with safe/dangerous scopes
- Approval dialog when triggering a dangerous scope without a grant
- The launcher's menu button (hamburger) opens the .desktop grid

## What needs the updater

- Natural-language input that falls back to Ollama (`qwen3:4b` not pulled)
- The updater itself is the next module — see `current-goals.md`.

## CI build

`.github/workflows/build-iso.yml` runs the same pipeline on every push to
a `v*.*.*` tag and attaches the ISO to the GitHub release. No need to
build locally if you just want the latest release artifact.
