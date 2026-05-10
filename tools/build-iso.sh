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

IMAGE_TAG="${IMAGE_TAG:-jarvis-os:dev}"
OUTPUT_DIR="${OUTPUT_DIR:-$(pwd)/iso/output}"
JARVIS_VERSION="${JARVIS_VERSION:-0.0.1}"

mkdir -p "$OUTPUT_DIR"

echo "─── Building OCI image: $IMAGE_TAG ───"
podman build \
    --file iso/Containerfile \
    --build-arg "JARVIS_VERSION=$JARVIS_VERSION" \
    --tag "$IMAGE_TAG" \
    .

echo "─── Converting to ISO via bootc-image-builder ───"
podman run --rm -it \
    --privileged \
    --pull=newer \
    --security-opt label=type:unconfined_t \
    -v "$OUTPUT_DIR":/output \
    -v ./iso/build.toml:/config.toml:ro \
    -v /var/lib/containers/storage:/var/lib/containers/storage \
    quay.io/centos-bootc/bootc-image-builder:latest \
    --type iso \
    --rootfs btrfs \
    --config /config.toml \
    "localhost/$IMAGE_TAG"

echo "─── Done ───"
echo "ISO:"
ls -lh "$OUTPUT_DIR"/bootiso/install.iso 2>/dev/null \
    || ls -lh "$OUTPUT_DIR"/*.iso 2>/dev/null \
    || echo "  (look under $OUTPUT_DIR)"
