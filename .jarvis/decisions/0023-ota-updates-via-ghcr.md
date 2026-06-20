# 0023 — OTA OS updates via a published ghcr image (`bootc upgrade`)

Status: accepted
Date: 2026-05-29

## Context

LilithOS is a bootc/ostree image-based system: the design intent is to
install an ISO **once**, then update by pulling a newer container image
and rebooting (`bootc upgrade`). The updater daemon, the UpdaterSplash
UI, the `Updater.ApplyOSUpgrade` DBus method, and the
`updater.check` / `updater.apply_os` Action Bus actions were all built
for this and work.

But the update channel was never connected. `build-iso.yml` built the OS
image as **`localhost/jarvis-os:<tag>`** (CI-local only), fed it to
bootc-image-builder to produce the ISO artifact, and stopped there. It
never pushed the OS image to a registry. So the installed system's bootc
origin pointed at `localhost/jarvis-os` — unreachable — and
`bootc upgrade` had nowhere to pull from. The only way to get new code
onto a VM was to rebuild the full ISO and **reinstall**, every time.
That is the opposite of what bootc is for.

## Decision

Publish the bootable OS image to **`ghcr.io/<owner>/jarvis-os`** and make
the installed system track it, so updates are OTA.

`build-iso.yml` changes:

1. **Trigger on push to `main`** (in addition to tags / dispatch). Every
   main push builds + publishes the image — the OTA channel stays
   current without anyone cutting a release.
2. **Tag the image with the registry ref**, not localhost:
   `ghcr.io/<owner>/jarvis-os:latest` (moving OTA tag) +
   `:<version>` (immutable record).
3. **Push the image to ghcr** on every build.
4. **Build the ISO from the ghcr ref** (`ghcr.io/<owner>/jarvis-os:latest`)
   so a freshly-installed system's bootc origin already points at the
   OTA channel.
5. **Gate the ISO steps** (bootc-image-builder + artifact + release)
   behind tags / manual dispatch only. The ISO is just the first-install
   medium; routine updates go OTA, so we don't pay the Anaconda ISO-build
   cost on every main push.

The flow becomes:

```
git push main  →  CI builds + pushes ghcr.io/<owner>/jarvis-os:latest
                  ↓
VM: open the update window (Updater.ApplyOSUpgrade → bootc upgrade)
                  ↓  pulls the new image, stages it
VM: reboot       →  boots the new image. No ISO, no reinstall.
```

The developer's machine builds nothing — the image build runs on GitHub.

## Operator requirements (one-time)

- The ghcr package **`jarvis-os` must be public** (GitHub → package
  settings), or every VM needs `podman login`. Public is the default
  choice.
- An **already-installed VM** whose origin is the old `localhost` ref
  must be re-pointed once: `sudo bootc switch ghcr.io/<owner>/jarvis-os:latest`
  (pulls the latest image + reboots). After that switch, the regular
  update window / `bootc upgrade` drives every future update.
- A **brand-new install** from a tag-built ISO already has the ghcr
  origin baked in — no switch needed.

## Consequences

- "Push code → wait for CI → click update in the VM" replaces "rebuild
  ISO locally → reinstall". No local build, no reinstall.
- VMs need network reach to ghcr.io to check/pull (they have it).
- `:latest` is a moving tag; bootc pins to the digest at deploy and
  compares the tag's digest on `upgrade --check`, so a new push reliably
  registers as "update available".
- Main pushes now consume GitHub Actions minutes for the image build
  (~8-12 min/push). If that becomes costly, narrow the trigger to
  `workflow_dispatch` + a `paths:` filter, or a manual "publish" button.
- The shell can't be built in WSL (Qt 6.4 vs the project's 6.5+), which
  is what made local dev-deploy of the shell awkward; OTA sidesteps that
  entirely — the Fedora toolchain that can build it lives in CI.

## Alternatives rejected

- **Local registry in WSL** — avoids the internet dependency but moves
  the ~8-15 min image build back onto the developer's machine and needs
  WSL↔VM networking set up. OTA via CI keeps the build off the local box.
- **dev-deploy.sh (scp a rebuilt binary)** — fine for Rust daemons, but
  the Qt shell needs the Fedora 42 / Qt 6.9 toolchain that WSL lacks, so
  it can't build the shell locally to push. Kept for daemon-only quick
  iterations; OTA is the general path.
