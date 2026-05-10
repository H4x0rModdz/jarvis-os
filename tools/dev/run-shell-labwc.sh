#!/usr/bin/env bash
# Boot a nested labwc compositor inside WSLg's Weston and launch the
# jarvis-shell binary as a layer-shell client.
#
# This is the only way to visually verify the Phase 1b layer-shell anchoring
# inside WSL2 — Weston itself does not implement wlr-layer-shell, so the bar
# would otherwise fall back to a regular toplevel window.
#
# Prerequisites (already in place if you followed the setup notes in
# memory/wsl_setup.md):
#   - labwc installed:                  sudo apt install labwc
#   - Qt 6.8.x via aqtinstall at:       ~/Qt/6.8.3/gcc_64
#   - LayerShellQt v6.0.0 built into:   ~/Qt/6.8.3/gcc_64
#   - jarvis-shell built into:          /tmp/jarvis-shell-build/
#     (CMAKE_PREFIX_PATH=$HOME/Qt/6.8.3/gcc_64 ; cmake -S shell/jarvis-shell
#      -B /tmp/jarvis-shell-build && cmake --build /tmp/jarvis-shell-build)
#
# Run from anywhere:
#     bash tools/dev/run-shell-labwc.sh

set -euo pipefail

SHELL_BIN="${SHELL_BIN:-/tmp/jarvis-shell-build/jarvis-shell}"
[[ -x "$SHELL_BIN" ]] || { echo "missing $SHELL_BIN — build jarvis-shell first"; exit 1; }
command -v labwc >/dev/null || { echo "labwc not installed — apt install labwc"; exit 1; }

# WSLg mounts /tmp/.X11-unix as a read-only tmpfs without the sticky bit,
# which causes wlroots' XWayland init to abort and take labwc with it.
# Replace it with a writable sticky-bit tmpfs for the lifetime of this run.
if [[ "$(stat -c '%A' /tmp/.X11-unix 2>/dev/null)" != *t ]]; then
    echo "Setting sticky bit on /tmp/.X11-unix (requires sudo)..."
    sudo umount /tmp/.X11-unix 2>/dev/null || true
    sudo mkdir -p /tmp/.X11-unix
    sudo chmod 1777 /tmp/.X11-unix
fi

# Kill any prior instances.
pkill -x jarvis-shell 2>/dev/null || true
pkill -x labwc        2>/dev/null || true
sleep 1

# Tell labwc to autostart the shell with its own software-rendering env so
# Mesa's ZINK probe doesn't refuse to initialize inside WSL.
mkdir -p ~/.config/labwc
cat > ~/.config/labwc/autostart <<EOF
LIBGL_ALWAYS_SOFTWARE=1 QT_QUICK_BACKEND=software "$SHELL_BIN" &
EOF

# Launch labwc as a nested wayland client of WSLg's Weston. WLR_WL_OUTPUTS=1
# gives one virtual output (1280x720 by default).
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$UID}"
export WLR_BACKENDS=wayland
export WLR_WL_OUTPUTS=1
export LIBGL_ALWAYS_SOFTWARE=1

echo "Launching labwc (host display=$WAYLAND_DISPLAY)..."
exec labwc
