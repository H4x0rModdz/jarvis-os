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

# Everything read above comes from registries we do not control, and it is
# about to be printed into a CI log, written into the Containerfile and placed
# in a commit message and a PR body by a job with write access to the repo.
# Treat it as hostile input: each value must match the exact shape it is
# supposed to have, or the script stops — without echoing the value.
require_shape() { # <name> <value> <extended regex>
    if [[ ! "$2" =~ $3 ]]; then
        echo "refusing to continue: $1 from the registry does not look like it should" >&2
        echo "(empty, or an upstream label changed format, or it was tampered with)." >&2
        exit 1
    fi
}
DIGEST_RE='^sha256:[0-9a-f]{64}$'
KERNEL_RE='^[0-9]+\.[0-9]+\.[0-9]+-[0-9A-Za-z._]+\.(x86_64|aarch64)$'
require_shape "the fedora-bootc digest"     "$BOOTC_DIGEST"  "$DIGEST_RE"
require_shape "the NVIDIA image digest"     "$NVIDIA_DIGEST" "$DIGEST_RE"
require_shape "the fedora-bootc kernel"     "$BOOTC_KERNEL"  "$KERNEL_RE"
require_shape "the NVIDIA image's kernel"   "$NVIDIA_KERNEL" "$KERNEL_RE"
# The driver version is informational (it only goes into a comment); if it is
# missing or odd, show "?" rather than fail the bump.
if [[ ! "$NVIDIA_DRIVER" =~ ^[0-9]+(\.[0-9]+){1,3}$ ]]; then
    NVIDIA_DRIVER="?"
fi

echo "fedora-bootc:$FEDORA_RELEASE          $BOOTC_DIGEST  kernel $BOOTC_KERNEL"
echo "akmods-nvidia-open:main-$FEDORA_RELEASE  $NVIDIA_DIGEST  kernel $NVIDIA_KERNEL  driver $NVIDIA_DRIVER"

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

# Rewrite the three lines. The new text is handed to awk through the
# environment and printed as-is — it is never part of a sed or awk program, so
# even a value that slipped past the checks above could not be interpreted as
# script syntax.
TMP=$(mktemp "${FILE}.XXXXXX")
trap 'rm -f -- "$TMP"' EXIT
BUMP_BOOTC_LINE="ARG FEDORA_BOOTC=$BOOTC_REPO@$BOOTC_DIGEST" \
BUMP_NVIDIA_LINE="ARG UBLUE_NVIDIA=$NVIDIA_REPO@$NVIDIA_DIGEST" \
BUMP_KERNEL_LINE="# kernel: $BOOTC_KERNEL   nvidia: $NVIDIA_DRIVER   resolved: $(date -u +%F)" \
awk '
    /^ARG FEDORA_BOOTC=/ { print ENVIRON["BUMP_BOOTC_LINE"];  next }
    /^ARG UBLUE_NVIDIA=/ { print ENVIRON["BUMP_NVIDIA_LINE"]; next }
    /^# kernel: /        { print ENVIRON["BUMP_KERNEL_LINE"]; next }
    { print }
' "$FILE" > "$TMP"
cat -- "$TMP" > "$FILE"

echo "Pins rewritten in $FILE:"
grep -n '^# kernel: \|^ARG FEDORA_BOOTC=\|^ARG UBLUE_NVIDIA=' "$FILE"
