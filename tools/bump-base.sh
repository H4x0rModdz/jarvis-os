#!/usr/bin/env bash
# tools/bump-base.sh — move the pinned base images forward, together.
#
# iso/Containerfile.runtime pins two images by digest (ADR 0033):
#
#   FEDORA_BOOTC   the OS base — this is where the kernel comes from
#   UBLUE_NVIDIA   Universal Blue's prebuilt, signed NVIDIA module + driver
#
# The module is built for one exact kernel, so the two digests are only valid
# as a pair. This script resolves what each tag points at right now, refuses
# to continue unless both are for the same kernel, and rewrites the pin lines.
# Review the diff, commit, open a PR; merging it rebuilds the runtime base.
#
# Run from the repo root:   tools/bump-base.sh           (rewrite the pins)
#                           tools/bump-base.sh --check   (report only)
#
# Exit status: 0 = pins rewritten or already current, 1 = error,
#              3 = upstream kernels differ (try again later).
#
# Needs `skopeo`; falls back to running it from a container with podman.
# To move to a new Fedora release, change FEDORA_RELEASE below — deliberately
# a manual, separate change.
set -euo pipefail

FEDORA_RELEASE=44
BOOTC_REPO=quay.io/fedora/fedora-bootc
NVIDIA_REPO=ghcr.io/ublue-os/akmods-nvidia-open
FILE=iso/Containerfile.runtime

CHECK_ONLY=0
[ "${1:-}" = "--check" ] && CHECK_ONLY=1

[ -f "$FILE" ] || { echo "run from the repo root ($FILE not found)" >&2; exit 1; }

run_skopeo() {
    if command -v skopeo >/dev/null 2>&1; then
        skopeo "$@"
    else
        podman run --rm quay.io/skopeo/stable:latest "$@"
    fi
}

# Prints "<digest> <label value>" for an image tag.
digest_and_label() { # <repo:tag> <label>
    run_skopeo inspect --format "{{.Digest}} {{index .Labels \"$2\"}}" "docker://$1"
}

read -r BOOTC_DIGEST BOOTC_KERNEL < <(digest_and_label "$BOOTC_REPO:$FEDORA_RELEASE" "ostree.linux")
# Universal Blue stamps "<kernel>-<builddate>" as the image version.
read -r NVIDIA_DIGEST NVIDIA_VERSION < <(digest_and_label "$NVIDIA_REPO:main-$FEDORA_RELEASE" "org.opencontainers.image.version")
NVIDIA_KERNEL=${NVIDIA_VERSION%-*}
NVIDIA_DRIVER=$(run_skopeo inspect --format '{{index .Labels "io.github.ublue-os.akmods.fingerprint-inputs"}}' \
                    "docker://$NVIDIA_REPO:main-$FEDORA_RELEASE" \
                | sed -n 's/.*"nvidia_akmod_evr":"[0-9]*:\([^"-]*\).*/\1/p')

for v in BOOTC_DIGEST BOOTC_KERNEL NVIDIA_DIGEST NVIDIA_KERNEL; do
    [ -n "${!v}" ] || { echo "could not resolve $v — did an upstream label change?" >&2; exit 1; }
done

echo "fedora-bootc:$FEDORA_RELEASE          $BOOTC_DIGEST  kernel $BOOTC_KERNEL"
echo "akmods-nvidia-open:main-$FEDORA_RELEASE  $NVIDIA_DIGEST  kernel $NVIDIA_KERNEL  driver ${NVIDIA_DRIVER:-?}"

if [ "$BOOTC_KERNEL" != "$NVIDIA_KERNEL" ]; then
    echo
    echo "Kernels differ: Fedora has published $BOOTC_KERNEL but the NVIDIA module is" >&2
    echo "built for $NVIDIA_KERNEL. Universal Blue usually catches up within a day —" >&2
    echo "nothing was changed; run this again later." >&2
    exit 3
fi

CURRENT_BOOTC=$(sed -n 's/^ARG FEDORA_BOOTC=.*@\(sha256:[0-9a-f]*\)$/\1/p' "$FILE")
CURRENT_NVIDIA=$(sed -n 's/^ARG UBLUE_NVIDIA=.*@\(sha256:[0-9a-f]*\)$/\1/p' "$FILE")
if [ -z "$CURRENT_BOOTC" ] || [ -z "$CURRENT_NVIDIA" ]; then
    echo "pin lines not found in $FILE — its format changed; fix this script" >&2
    exit 1
fi

if [ "$CURRENT_BOOTC" = "$BOOTC_DIGEST" ] && [ "$CURRENT_NVIDIA" = "$NVIDIA_DIGEST" ]; then
    echo "Already current."
    exit 0
fi

if [ "$CHECK_ONLY" = 1 ]; then
    echo "Update available (not applied: --check)."
    exit 0
fi

sed -i \
    -e "s|^ARG FEDORA_BOOTC=.*|ARG FEDORA_BOOTC=$BOOTC_REPO@$BOOTC_DIGEST|" \
    -e "s|^ARG UBLUE_NVIDIA=.*|ARG UBLUE_NVIDIA=$NVIDIA_REPO@$NVIDIA_DIGEST|" \
    -e "s|^# kernel: .*|# kernel: $BOOTC_KERNEL   nvidia: ${NVIDIA_DRIVER:-?}   resolved: $(date -u +%F)|" \
    "$FILE"

echo "Pins rewritten in $FILE:"
grep -n '^# kernel: \|^ARG FEDORA_BOOTC=\|^ARG UBLUE_NVIDIA=' "$FILE"
