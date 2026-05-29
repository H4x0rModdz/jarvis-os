# ADR 0021 — ISO build speed: cache mounts now, prebuilt builder next

**Status:** Accepted (P1 implemented). P2/P3 proposed.
**Date:** 2026-05-21

## Problem

A full ISO build runs 30–60 min cold. The dominant cost is that
`COPY . .` in the builder stage invalidates the `cargo build` layer
on *any* source change, so all 14 crates **and the entire dependency
tree** (tokio, zbus, reqwest, rusqlite, rustfft, smithay, …)
recompile from zero. A one-character edit to Lilith pays the full
cold-compile tax. whisper.cpp and numbat-cli also rebuild from source
every time, and the dnf devel-deps reinstall every time.

This kills iteration: testing a small change against a real boot
shouldn't cost half an hour. (The `dev-deploy.sh` fast loop from the
devx work covers most day-to-day testing without an ISO at all — this
ADR is about the periodic *full image* build still being painful.)

## Decision

Three layers of fix, by ROI:

### P1 — cargo/whisper/numbat cache mounts (DONE)

`RUN --mount=type=cache` on the heavy compile steps:
- cargo target dir (`/build/rust`, `/build/numbat`) + the cargo
  registry/git (`/root/.cargo/{registry,git}`)
- whisper cmake build dir (`/build/whisper`)

Cache mounts persist **across builds** but never land in an image
layer. A source-only edit recompiles just our crates; the dep tree
stays warm. Because a cache-mounted dir is gone once its RUN ends,
the binary-collection `cp` moved *into* the same RUN as each build
(the standalone "collect" step now only handles the cmake outputs,
which live in ordinary layers).

Effect: **local** rebuilds drop the Rust step from ~15 min to ~2 min
immediately (podman keeps cache mounts on the host between builds).

Caveat: cache mounts do **not** persist across fresh CI runners by
themselves — that needs P3.

### P2 — prebuilt `jarvis-builder` base image (PROPOSED)

Bake Fedora + all `*-devel` deps + a prebuilt whisper-cli + numbat
into `ghcr.io/<org>/jarvis-builder:<tag>`, built by its own workflow
and pushed once. The main Containerfile's builder stage becomes
`FROM ghcr.io/<org>/jarvis-builder`. Removes the dnf install
(~2-3 min), whisper build (~3-5 min) and numbat build (~2-4 min)
from *every* ISO build — they only re-run when the builder image is
rebuilt (rarely: a dep bump or a whisper/numbat version bump).

Cost: one new workflow (`build-builder.yml`), a second Containerfile
(`iso/Containerfile.builder`), and a `ghcr.io` push (needs
`packages: write`, already granted in build-iso.yml).

### P3 — CI cache wiring (PROPOSED, pairs with P2)

Make the CI runner benefit from P1's cache mounts + layer cache:
- `--cache-from` / `--cache-to` against the ghcr builder + a
  `jarvis-os:cache` tag so buildah layers restore across runs.
- `actions/cache` keyed on `Cargo.lock` hash for the cache-mount
  export dirs if we move to a self-hosted or cache-mount-exporting
  setup.

Effect: warm CI rebuilds approach the local times.

## Out of scope / unchanged

- `bootc-image-builder` (OCI → ISO, ~5-10 min) is a fixed cost; not
  cacheable in any meaningful way. It only runs after the OCI image
  is assembled.
- The Flatpak installs (Firefox/Dolphin/Zed) are in the final stage
  and already layer-cache locally as long as no earlier final-stage
  line churns. P4 (reorder for cache stability) is a minor follow-up
  if it ever becomes the bottleneck.

## Consequences

- The builder stage's `cp`-to-`/out` is now coupled to each build
  RUN (can't read a cache mount from a later step). New binaries
  must be copied to `/out` inside the RUN that builds them — a small
  invariant to remember when adding a daemon.
- P2 introduces a versioned builder image the team must rebuild when
  bumping toolchain / whisper / numbat. The version pin lives in the
  builder workflow.
