#!/usr/bin/env bash
# Build a bootable Jarvis OS ISO from this repo.
#
# Pipeline:
#   1. `podman build` produces an OCI image from iso/Containerfile.
#   2. `bootc-image-builder` (a container) takes that image and writes an
#      ISO under ./iso/output/.
#
# Run from repository root:
#     bash tools/build-iso.sh
#
# Requirements:
#   - podman 4.5+ (rootless OK)
#   - ~10 GB free disk for build artifacts
#   - Internet access (pulls Fedora + wayblue base images)

set -euo pipefail

# Tee everything to a log next to the repo (Windows-visible since the
# repo lives on /mnt/c). When the build dies — including a hard crash
# that closes the terminal — the full output survives in build-iso.log
# for diagnosis. LOG= overrides the path; LOG=/dev/null disables.
LOG="${LOG:-$(pwd)/build-iso.log}"
if [ "$LOG" != "/dev/null" ]; then
    exec > >(tee "$LOG") 2>&1
    echo "─── log: $LOG ───"
fi

# The image reference baked into the installed system as its bootc origin —
# i.e. where `bootc upgrade` (and the in-shell "system update") pulls from.
# Default to the ghcr OTA channel that CI publishes, so an OS installed from a
# LOCALLY-built ISO tracks the SAME updates as everyone else instead of a
# dead localhost tag. The ISO still ships YOUR local build (see --pull=never
# below); only the *origin* points at ghcr, so the first `bootc upgrade`
# converges onto CI's published image. Override with IMAGE=... for a pure
# local image that never auto-updates. (ghcr requires a lowercase owner.)
IMAGE="${IMAGE:-ghcr.io/h4x0rmoddz/jarvis-os:latest}"
OUTPUT_DIR="${OUTPUT_DIR:-$(pwd)/iso/output}"
JARVIS_VERSION="${JARVIS_VERSION:-0.0.1}"
# Builder base (ADR 0021 P2). The main Containerfile defaults to the
# ghcr image; for a self-sufficient local build we build it here and
# point at the local tag if a copy isn't already present.
BUILDER_IMAGE="${BUILDER_IMAGE:-localhost/jarvis-builder:local}"

mkdir -p "$OUTPUT_DIR"

# ── Builder base ─────────────────────────────────────────────────────
# Reuse an existing builder image if we have one (it changes rarely —
# only on toolchain / whisper / piper / numbat bumps). Build it
# otherwise. Set REBUILD_BUILDER=1 to force a fresh one.
if [ "${REBUILD_BUILDER:-0}" = "1" ] \
        || ! podman image exists "$BUILDER_IMAGE" 2>/dev/null; then
    echo "─── Building builder base: $BUILDER_IMAGE ───"
    podman build \
        --file iso/Containerfile.builder \
        --tag "$BUILDER_IMAGE" \
        .
else
    echo "─── Reusing builder base: $BUILDER_IMAGE (REBUILD_BUILDER=1 to force) ───"
fi

echo "─── Building OCI image: $IMAGE ───"
podman build \
    --file iso/Containerfile \
    --build-arg "JARVIS_VERSION=$JARVIS_VERSION" \
    --build-arg "BUILDER_IMAGE=$BUILDER_IMAGE" \
    --tag "$IMAGE" \
    .

echo "─── Converting to ISO via bootc-image-builder ───"
# --pull=never: use the image we JUST built in local (rootful) storage as the
# ISO payload, NOT whatever is on ghcr. The installed system's origin is still
# the ghcr ref ("$IMAGE"), so it OTA-upgrades from ghcr later — but the ISO
# itself carries your local build. (CI does the opposite: it pushes first,
# then pulls, because there the registry copy IS the source of truth.)
podman run --rm -it \
    --privileged \
    --pull=never \
    --security-opt label=type:unconfined_t \
    -v "$OUTPUT_DIR":/output \
    -v ./iso/build.toml:/config.toml:ro \
    -v /var/lib/containers/storage:/var/lib/containers/storage \
    quay.io/centos-bootc/bootc-image-builder:latest \
    --type iso \
    --rootfs btrfs \
    --config /config.toml \
    "$IMAGE"

echo "─── Done ───"
echo "ISO:"
ls -lh "$OUTPUT_DIR"/bootiso/install.iso 2>/dev/null \
    || ls -lh "$OUTPUT_DIR"/*.iso 2>/dev/null \
    || echo "  (look under $OUTPUT_DIR)"
