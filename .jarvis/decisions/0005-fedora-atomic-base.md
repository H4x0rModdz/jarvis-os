# ADR 0005: Fedora Atomic as Distribution Base (OCI Image Model)

## Status
Accepted

## Context

LilithOS needs a Linux distribution base. The candidates evaluated were:
- Fedora (traditional)
- NixOS
- Arch Linux
- Fedora Atomic (OCI image model)

The core requirements:
- Modern Wayland support
- Wine/Proton compatibility (gaming-grade)
- Reproducible, versionable OS releases
- Rollback capability for end users
- Accessible to non-technical users eventually
- AI-readable system definition

## Decision

Use **Fedora Atomic** as the base, distributing LilithOS as an **OCI image** built on top of it.

The build system is **BlueBuild** (or a custom Containerfile pipeline).

## How It Works

```
Fedora Atomic base (OCI image)
  └── LilithOS Containerfile
        ├── Remove default desktop environment
        ├── Add Qt6, wlroots, Vulkan tooling
        ├── Add Jarvis shell (compositor + window manager + taskbar)
        ├── Add Lilith daemon + Ollama
        ├── Add Action Bus + Permission System
        └── Add Wine/Proton stack
```

LilithOS releases are versioned OCI images. Updates are atomic. Rollback is built-in.

## Reasons

**Immutable base:**
The system root is read-only. Users cannot accidentally break the OS. Jarvis-specific
mutable state lives in `/var` and `~/.jarvis/`.

**Atomic updates:**
Updates ship as new image versions. No partial-update failures. If an update breaks
something, one command rolls back to the previous image.

**Reproducibility:**
The entire OS definition is a Containerfile — version-controlled, reviewable, AI-readable.
Any developer can reproduce the exact OS image from the repo.

**Proven model for gaming/compatibility:**
Bazzite (gaming Linux) uses this exact model on Fedora Atomic. It achieved production-grade
Wine/Proton/Steam integration. We inherit that work.

**Fedora's Wayland ecosystem:**
Red Hat is the primary contributor to Wayland. Fedora Atomic ships the most mature
Wayland stack available on Linux.

## Consequences

- LilithOS dev environment is defined in a Containerfile (good: reproducible, AI-readable)
- System updates require an image rebuild + reboot (acceptable for a desktop OS)
- `/usr` is immutable — Jarvis system files go there at image build time, not at runtime
- User data and config live in `/var/home` and `~/.jarvis/` (mutable layer)
- Flatpak is the primary app delivery mechanism on top of the immutable base
- Wine/Proton prefixes live in the mutable user layer (`~/.jarvis/compat/`)

## Reference Implementations

- **Bazzite**: gaming OS on Fedora Atomic — proves Wine/Proton/Steam viability
- **BlueBuild**: framework for custom Fedora Atomic images
- **Universal Blue**: org maintaining the ecosystem

## Alternatives Rejected

- **Fedora traditional**: no immutability, no atomic updates, breaks like any standard distro
- **NixOS**: better reproducibility model but steep learning curve, smaller hardware/driver ecosystem, Wine quirks
- **Arch**: excellent for personal use, wrong foundation for a distributable OS

## Revalidation — 2026-11-23

**Question:** after the real-hardware arc (software rendering on an RTX 3060,
the NVIDIA driver saga of ADR 0032, a shell that stuttered), is Fedora the
wrong base?

**Answer: no — keep it.** Each problem was traced to its cause, and almost
none of them live in the distribution:

| Problem | Root cause | Would another base fix it? |
|---|---|---|
| Whole desktop software-rendered on a real GPU | Our own `labwc/environment` forced pixman + llvmpipe everywhere (PR #51) | No — our file |
| Still llvmpipe after that | `fedora-bootc` is a minimal base: no GPU firmware, no Vulkan drivers (PR #49) | No — a choice of *which Fedora image*; any minimal base has it |
| No accelerated OpenGL on NVIDIA with the open stack | Upstream Mesa (nouveau/NVK+Zink on Ampere) | No — same Mesa everywhere |
| Proprietary NVIDIA driver is fiddly (akmod at image build, `kernel-devel` pin, unsigned module vs Secure Boot) | Fedora keeps it out of tree and out of the main repos | **Partly** — Arch/Ubuntu/openSUSE package it more directly |
| wlroots needs NVIDIA env vars and a software cursor | The compositor (labwc/wlroots), not the distro | No |
| Shell hitches and lag | Our shell: blocking I/O on the GUI thread, polling, delegate churn, re-blurred panels | No — our code |

So one row out of six is Fedora's, and it is a packaging inconvenience that is
already solved in the image. Against that, leaving Fedora would cost the things
the OS is built on: bootc OTA + rollback (ADR 0023, the updater daemon), the
Containerfile pipeline and prebuilt bases (ADR 0021), and Fedora's current
Qt/Mesa/Wayland (the shell needs Qt 6.9+).

**Alternatives, re-checked:**

- **Arch** — best NVIDIA packaging, newest everything, but no supported
  image-based/atomic model; we would be building our own OTA and rollback.
- **Ubuntu** — NVIDIA is easy, but no bootc, and an LTS Qt that is too old for
  the shell; we would end up shipping our own Qt.
- **NixOS** — reproducibility as good as ours, at the price of rewriting the
  entire image definition in a language none of the tooling here speaks.
- **openSUSE Aeon/Kalpa** — atomic, but transactional-update rather than OCI
  images; a smaller ecosystem to borrow from.
- **A Universal Blue image as the base** — not leaving Fedora at all; see below.

**The real costs of Fedora, to manage rather than escape:**

- A release every six months, each supported ~13 months — rebasing is a
  recurring tax (42 → 44 already), and each one can move the kernel, Mesa and Qt
  under us. The shell links Qt private API (`WaylandClientPrivate`), which has
  no compatibility promise between minors.
- NVIDIA stays out-of-tree: the module is built per image and unsigned.
- Codecs and the NVIDIA driver come from RPM Fusion, a third-party repo.

**Follow-ups that are worth more than a rebase** (1 and 2 done — ADR 0033):

1. **Revisit ADR 0032's rejection of Universal Blue's prebuilt, signed NVIDIA
   kmods** (or building `FROM` a ublue base). It removes the `kernel-devel`
   pin and answers Secure Boot, which is the one genuinely Fedora-shaped pain.
2. **Pin `fedora-bootc` by digest** and bump it deliberately, so a base refresh
   is a reviewed change instead of whatever `:44` resolved to that day.
3. **Treat the compositor as the graphics variable it is** (P001). How well
   NVIDIA works is decided far more by labwc/wlroots vs. the alternatives than
   by anything under it.
