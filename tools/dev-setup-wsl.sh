#!/usr/bin/env bash
# Jarvis OS — WSL2 Development Environment Setup
# Run this INSIDE WSL2 after installing it:
#   bash tools/dev-setup-wsl.sh

set -e

echo "==> Jarvis OS WSL2 dev environment setup"

# ── System dependencies ───────────────────────────────────────────────────
echo "==> Installing system packages..."
sudo apt update -qq
sudo apt install -y \
    build-essential \
    pkg-config \
    git \
    curl \
    \
    # Wayland
    libwayland-dev \
    libwayland-egl-backend-dev \
    wayland-protocols \
    \
    # Smithay dependencies
    libseat-dev \
    libgbm-dev \
    libdrm-dev \
    libinput-dev \
    libudev-dev \
    libxkbcommon-dev \
    \
    # OpenGL / EGL (for GlesRenderer + winit backend)
    libgl1-mesa-dev \
    libegl1-mesa-dev \
    libgles2-mesa-dev \
    \
    # Winit backend (X11 fallback for WSL2 if WSLg Wayland isn't available)
    libx11-dev \
    libxrandr-dev \
    libxi-dev \
    libxcursor-dev \
    \
    # DBus (Action Bus)
    libdbus-1-dev \
    \
    # Notify-send (for system.notify handler)
    libnotify-bin

echo "==> System packages installed"

# ── Rust ──────────────────────────────────────────────────────────────────
if ! command -v rustup &>/dev/null; then
    echo "==> Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    source "$HOME/.cargo/env"
else
    echo "==> Rust already installed: $(rustc --version)"
    rustup update stable
fi

source "$HOME/.cargo/env"

# ── Verify ────────────────────────────────────────────────────────────────
echo ""
echo "==> Versions:"
rustc --version
cargo --version
pkg-config --modversion wayland-server 2>/dev/null || echo "  wayland-server: check manually"

# ── Build check ───────────────────────────────────────────────────────────
echo ""
echo "==> Building Action Bus..."
cargo build -p jarvis-action-bus 2>&1

echo ""
echo "==> Building Compositor..."
cargo build -p jarvis-compositor 2>&1

echo ""
echo "======================================================"
echo "  Setup complete!"
echo ""
echo "  To run the compositor (winit dev backend):"
echo "    cargo run -p jarvis-compositor"
echo ""
echo "  To run the Action Bus daemon:"
echo "    cargo run -p jarvis-action-bus"
echo "======================================================"
