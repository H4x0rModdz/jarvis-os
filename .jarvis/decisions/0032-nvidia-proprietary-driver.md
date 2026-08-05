# ADR 0032 — Ship the NVIDIA driver in the image

**Status:** accepted
**Date:** 2026-08-04
**Supersedes in part:** the "open stack only" position taken in ADR 0031's GPU note

## Context

On real hardware (RTX 3060 / Ampere, driving an ultrawide) the desktop was
unusable. The investigation found two separate faults:

1. **Ours.** `/etc/xdg/labwc/environment` hardcoded `WLR_RENDERER=pixman` and
   `LIBGL_ALWAYS_SOFTWARE=1` "for VMs", unconditionally, on every machine. Both
   the compositor and every Qt window were pinned to the CPU regardless of the
   GPU. Fixed separately (PR #51) — necessary, but not sufficient.
2. **The open NVIDIA stack.** With that removed, Vulkan works (NVK sees the
   RTX correctly) but OpenGL still lands on `llvmpipe`, `Accelerated: no`. The
   `nouveau` GL driver never supported Ampere; since Mesa 25.1 the GL path for
   Turing+ is NVK + Zink, and that selection is not happening here. Forcing
   `MESA_LOADER_DRIVER_OVERRIDE=zink` produced Zink running on **lavapipe** —
   software Vulkan. Mesa's Vulkan device-select layer is known to be disabled
   in some Zink paths because of deadlocks (airlied, Nov 2025), so Zink takes
   whichever device enumerates first.

Qt cannot route around it either: `QSG_RHI_BACKEND=vulkan` fails with
`Failed to create RHI (backend 1)`.

We are also going to want **CUDA** — running Lilith's models on the GPU is on
the roadmap, and the open stack has no CUDA at all.

## Decision

Ship the **NVIDIA driver from RPM Fusion**, baked into the runtime base image.

Install `akmod-nvidia` — RPM Fusion's mainstream path, which on a supported GPU
already builds NVIDIA's **open kernel modules**, [what NVIDIA recommends for
Turing and newer and the default since driver 560][nv-open]. "Open" describes
the *kernel module* only; GL, Vulkan and CUDA userspace are the proprietary blob
either way. We do not take the tainted repo's explicit `akmod-nvidia-open` swap:
on this hardware it yields the same modules while adding a repo and a swap step
we cannot verify without the hardware.

The kmod is compiled **at image build time** against the kernel in the image
(`akmods --force --kernels <kver>`). This is the correct model for bootc: the
kernel ships inside the image, so the module is always built against exactly
the kernel it will boot, and every base rebuild rebuilds it. There is no
"module didn't build after a kernel update" failure mode on the user's machine.

Kernel arguments (`nvidia-drm.modeset=1`, nouveau blacklist) ship via
`/usr/lib/bootc/kargs.d/`, which bootc applies at install **and** on upgrade.

[nv-open]: https://developer.nvidia.com/blog/nvidia-transitions-fully-towards-open-source-gpu-kernel-modules.md/

## Consequences

### Accepted costs

- **Not free software.** The userspace driver is a proprietary blob from a
  third-party repo (RPM Fusion nonfree). This is a real departure from the
  project's posture. It does not affect our GPL-3.0 licensing — we neither link
  against nor redistribute a modified NVIDIA driver, we install a package — but
  it does mean the image is no longer wholly free software, and that should be
  stated plainly to users.
- **Image size** grows by roughly a gigabyte, on every install and every OTA.
- **Build time** grows: compiling the kmod is minutes per base rebuild. Only on
  base rebuilds, not per merge.
- **Secure Boot.** An unsigned out-of-tree module will not load with Secure
  Boot enabled. Users must either disable Secure Boot or enrol a MOK. This is
  a genuine wart on a dual-boot machine where Windows expects Secure Boot on.
  Not solved here; see Follow-ups.
- **Every machine gets it**, including AMD/Intel boxes and VMs, where the
  module simply never loads. One image is worth more than a matrix of variants
  at this stage; a `-nvidia` variant can come later if size becomes the problem.

### What we keep

`mesa-vulkan-drivers` and the AMD/Intel GPU firmware stay: they are what makes
non-NVIDIA hardware work, and they are why an AMD machine won't repeat this.

## Alternatives rejected

- **Stay on nouveau + NVK + Zink.** The open path is the principled choice and
  we tried it first. It does not currently deliver working OpenGL on Ampere
  here, and it will never deliver CUDA. Continuing to hold the line was costing
  the project its own daily driver.
- **A separate `jarvis-os-nvidia` image variant** (the Universal Blue model).
  Correct at scale, but doubles CI for a project whose image build has already
  been fighting for disk. Revisit if the size cost bites.
- **Prebuilt akmods from `ghcr.io/ublue-os/akmods`.** This is what Universal
  Blue itself does — `COPY --from=ghcr.io/ublue-os/akmods-nvidia:main-44` and
  install the ready-made `kmod-nvidia` RPMs — precisely because matching
  `kernel-devel` to the running kernel is the fragile step (they go further and
  ship their own kernel RPMs, `versionlock`ed with `kernel-devel`). It saves
  build time, but ties us to another project's kernel and Fedora tags, and a
  mismatch produces a module that silently refuses to load. Building against
  our own kernel keeps the image self-contained; the `modinfo` check makes the
  mismatch a loud build failure instead of a silent one.

## Follow-ups

- **Sign the kmod with our own key + document MOK enrolment**, so Secure Boot
  can stay on. Universal Blue does exactly this — they ship the certificates
  (`ubmok101.cer`, `ubmok102.cer`) in the image for users to enrol — which is
  the pattern to copy. Until then, Secure Boot must be off, and on a dual-boot
  machine that is a real imposition.
- Point Ollama at CUDA now that it exists (currently CPU-only).
- Revisit the `-nvidia` image split if OTA size becomes painful.

## Related

- `iso/Containerfile.runtime`, `iso/assets/bootc/kargs.d/10-nvidia.toml`
- ADR 0021 (two-base build — why this lands in the runtime base)
- PR #49 (GPU firmware + NVK), PR #51 (stop forcing software rendering)
