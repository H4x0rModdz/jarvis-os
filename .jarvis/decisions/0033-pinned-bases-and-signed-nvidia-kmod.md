# ADR 0033 — Pin the base images by digest; take the NVIDIA module prebuilt and signed

**Status:** accepted
**Date:** 2026-11-23
**Amends:** ADR 0032 (which driver we ship is unchanged; how we obtain it is not)

## Context

Two follow-ups from the ADR 0005 revalidation, which turned out to be one
problem.

1. **The base was a moving tag.** `FROM quay.io/fedora/fedora-bootc:44` is
   whatever Fedora pushed most recently. Every runtime rebuild could change the
   kernel, Mesa and the driver stack with no diff anywhere to review or revert.
2. **The NVIDIA module was built by us, unsigned** (ADR 0032): `akmod-nvidia`
   compiled at image build against `kernel-devel` for the image's exact kernel.
   That left two known warts — it cannot load with Secure Boot on, and the
   build breaks whenever Fedora's repos no longer carry `kernel-devel` for the
   kernel in the base (they keep only the newest).

Pinning the base makes the second wart *worse*: a pinned kernel is by
definition not the newest, so its `kernel-devel` disappears from the repos
within days. The two had to be solved together.

ADR 0032 rejected Universal Blue's prebuilt modules because they "tie us to
another project's kernel", and a mismatch "produces a module that silently
refuses to load". Both objections were checked against the actual artifacts
(`ghcr.io/ublue-os/akmods-nvidia-open:main-44`, inspected 2026-11-23):

- Their `main` kernel **is Fedora's kernel build** — same version, and the same
  `vmlinuz` carrying Fedora's Secure Boot signature plus theirs. A module built
  for it has the vermagic of the stock kernel `fedora-bootc` ships. We do not
  need their kernel at all.
- The module is **signed** (`signer: ublue kernel`), and the image carries the
  matching userspace driver RPMs and the install script their own images use.
- The mismatch risk is real, but it is a property of *unpinned* inputs.

## Decision

1. **Pin both inputs by digest** in `iso/Containerfile.runtime`:
   `FEDORA_BOOTC` (the OS base, source of the kernel) and `UBLUE_NVIDIA`
   (the prebuilt module + driver). They are valid only as a pair.
2. **Install the NVIDIA driver from the Universal Blue image** by running their
   `nvidia-install.sh` against the mounted RPMs, instead of compiling with
   akmods. We keep the stock Fedora kernel. RPM Fusion is no longer used.
3. **Verify at build time, loudly:**
   - the base's `kernel-core` equals the kernel the module was built for;
   - the module is signed, and the certificate we ship
     (`/etc/pki/akmods/certs/akmods-ublue.der`) is the one that signed it
     (modinfo `sig_key` = certificate serial). An upstream key rotation fails
     the build instead of shipping a certificate that enrols nothing.
4. **Move the pins with a tool, never by hand:** `tools/bump-base.sh` resolves
   both tags, refuses to proceed unless their kernels match, and rewrites the
   lines. `.github/workflows/bump-base.yml` runs it weekly and opens a PR that
   states the kernel and driver the OS is about to change to. Merging it
   rebuilds the runtime base.

### Secure Boot, for the user

With Secure Boot on, enrol the certificate once:

```
sudo mokutil --import /etc/pki/akmods/certs/akmods-ublue.der
# choose a one-time password → reboot → "Enroll MOK" on the blue screen
```

Until then (or with Secure Boot off) behaviour is what it was before: the
module does not load under Secure Boot, and loads without it. The kernel itself
is still Fedora-signed, so the machine boots either way.

## Consequences

**Gained**

- A base refresh is a reviewed, revertible one-line diff with the kernel and
  driver version in it.
- Secure Boot can stay on (after one enrolment) — the dual-boot wart of ADR
  0032.
- No compiler, `kernel-devel` or akmods in the image build; no dependence on a
  package Fedora deletes.
- We ship exactly the kernel-module binary that Bazzite, Bluefin and Aurora
  ship — the most-tested NVIDIA-on-Fedora-atomic artifact there is.

**Accepted costs**

- **We trust Universal Blue's build and signing key** for a kernel module. It
  is a third party with root-equivalent reach into the image. Mitigated by the
  digest pin (nothing changes without a PR), not eliminated.
- **Updates wait for two upstreams.** When Fedora publishes a kernel before
  Universal Blue has built for it, `bump-base.sh` exits 3 and that week's bump
  is skipped. Security updates to the kernel can lag by that gap.
- **Updates are no longer automatic.** Someone has to merge the weekly PR.
  That is the point of pinning, but it is a duty that did not exist before.
- The driver userspace is now negativo17's packaging (via their repo files),
  not RPM Fusion's. Package names differ (`nvidia-driver*`); their install
  script also adds `nvidia-container-toolkit` and disables the
  `fedora-cisco-openh264` repo.
- Their package no longer ships the `nvidia-suspend/-resume/-hibernate` units
  we used to enable; `NVreg_PreserveVideoMemoryAllocations=1` comes from their
  `modprobe.d/nvidia.conf`. **Suspend/resume must be re-verified on hardware.**

**Not changed:** kernel arguments (`kargs.d/10-nvidia.toml`), the session
environment for wlroots (`jarvis-session-launch`), 32-bit driver libraries
(still not installed — `MULTILIB=0`).

## Verification

Built locally (`fedora-bootc` + GPU firmware + the NVIDIA step + certificate
check, the rest of the runtime omitted): build green; `kmod-nvidia`,
`nvidia-driver`, `nvidia-driver-cuda` 620.73.04; `modinfo nvidia` →
`signer: ublue kernel`, vermagic `7.3.4-200.fc44.x86_64`; NVIDIA GBM and EGL
vendor files present. **Not verified:** booting it on the RTX 3060 — that
needs the image.

## Alternatives rejected

- **Sign our own akmod build with our own key.** Fixes Secure Boot, keeps the
  `kernel-devel` fragility (worse under pinning), and adds a private signing
  key to manage in CI that local builds would not have.
- **Fetch `kernel-devel` from Koji by exact NVR** to keep building ourselves.
  Fixes the pin fragility, not Secure Boot.
- **Adopt Universal Blue's kernel RPMs too.** Unnecessary — it is the same
  kernel — and it would mean regenerating the initramfs in our build.
- **Build `FROM` a Universal Blue desktop image.** Inherits a GNOME/KDE
  desktop we would then strip back out.
- **Pin only the base and keep the tag for the module.** The pair would drift
  apart on the first kernel update and fail the build until someone noticed.

## Related

- `iso/Containerfile.runtime`, `tools/bump-base.sh`,
  `.github/workflows/bump-base.yml`, `iso/assets/secureboot/akmods-ublue.der`
- ADR 0005 (revalidation, 2026-11-23), ADR 0021 (two-base build), ADR 0032
