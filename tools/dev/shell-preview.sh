#!/usr/bin/env bash
# tools/dev/shell-preview.sh — build the shell and LOOK at it, without an ISO.
#
#   tools/dev/shell-preview.sh live             a live window with the shell
#                                               in it (needs WSLg / a Wayland
#                                               session on the host)
#   tools/dev/shell-preview.sh shot [name]      one screenshot → .preview/<name>.png
#
# Everything runs in a container (podman): the shell is compiled with the same
# Qt the OS ships and started under a real labwc with the session's rc.xml.
# Rendering is software, so this shows what the shell LOOKS like and whether
# QML loads cleanly (.preview/shell.log) — it says nothing about performance
# on a GPU, and none of the daemons (Lilith, Settings, …) are running.
#
# Environment: RES=2560x1080 (output size), SETTLE=6 (seconds before a shot),
# JARVIS_SHELL_SURFACES, PREVIEW_APPS=0 (no sample window),
# PREVIEW_HOOK="cmd" (run inside the session just before the shot).
#
# From Windows:  wsl -d Ubuntu-24.04 bash -lc "cd /mnt/c/dev/jarvis-os && tools/dev/shell-preview.sh live"
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
IMAGE=localhost/lilith-shell-preview:latest
BUILD_DIR="${LILITH_SHELL_BUILD:-$HOME/lilith-shell-build}"
MODE="${1:-shot}"
NAME="${2:-shell}"

mkdir -p "$BUILD_DIR" "$REPO/.preview"

# (Re)build the preview image when its Containerfile changed.
WANT=$(sha256sum "$REPO/tools/dev/shell-preview/Containerfile" | cut -d' ' -f1)
HAVE=$(podman image inspect "$IMAGE" --format '{{ index .Labels "lilith.preview-hash" }}' 2>/dev/null || true)
if [ "$WANT" != "$HAVE" ]; then
    echo "Building the preview image (one-off, a few minutes)…"
    podman build -q --label "lilith.preview-hash=$WANT" -t "$IMAGE" \
        -f "$REPO/tools/dev/shell-preview/Containerfile" "$REPO/tools/dev/shell-preview" >/dev/null
fi

ARGS=(--rm
    -v "$REPO:/src:ro" -v "$BUILD_DIR:/build" -v "$REPO/.preview:/out"
    -e MODE="$MODE" -e NAME="$NAME"
    -e RES="${RES:-2560x1080}" -e SETTLE="${SETTLE:-6}"
    -e PREVIEW_APPS="${PREVIEW_APPS:-1}" -e PREVIEW_HOOK="${PREVIEW_HOOK:-}")
[ -n "${JARVIS_HUD:-}" ] && ARGS+=(-e JARVIS_HUD=1)
[ -n "${JARVIS_SHELL_SURFACES:-}" ] && ARGS+=(-e JARVIS_SHELL_SURFACES="$JARVIS_SHELL_SURFACES")

case "$MODE" in
    live)
        SOCK="${XDG_RUNTIME_DIR:-/mnt/wslg/runtime-dir}/${WAYLAND_DISPLAY:-wayland-0}"
        [ -S "$SOCK" ] || { echo "no host Wayland socket at $SOCK (WSLg not running?)" >&2; exit 1; }
        ARGS+=(-v "$SOCK:/tmp/xdg/host-wayland" -e RES="${RES:-1920x1080}")
        echo "Opening a window with the shell in it. Close the window (or Ctrl+C) to stop."
        ;;
    shot) ;;
    *) echo "usage: $0 live | shot [name]" >&2; exit 2 ;;
esac

podman run "${ARGS[@]}" "$IMAGE" sh /src/tools/dev/shell-preview/session.sh 2>&1 | grep -v 'level=warning'
[ "$MODE" = shot ] && echo "→ $REPO/.preview/$NAME.png"
