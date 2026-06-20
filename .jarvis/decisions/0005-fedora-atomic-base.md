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
