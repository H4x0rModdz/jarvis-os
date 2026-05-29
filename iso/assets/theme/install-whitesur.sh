#!/usr/bin/env bash
# Install the WhiteSur (macOS-like) GTK theme + icon theme + cursors
# system-wide into /usr/share. Run at image build time from the
# Containerfile.
#
# Scope, deliberately narrow:
#   - GTK theme  → styles GTK apps (Firefox via Flatpak). NOT our Qt
#     shell, which keeps its own glassmorphic design language.
#   - Icon theme → launcher grid + file manager + every app that
#     reads the XDG icon theme. Biggest visual payoff.
#   - Cursors    → the macOS-style pointer, applied session-wide via
#     XCURSOR_THEME in jarvis-session-launch.
#
# Dolphin (Qt/KDE) is NOT styled by a GTK theme — matching it needs
# the separate WhiteSur Kvantum theme. Deferred; documented in the
# Containerfile.
#
# We clone the default branch shallow rather than pinning a tag: the
# WhiteSur repos don't keep a stable tag cadence and a 404'd tag would
# break the whole image build. Reproducibility trade-off accepted for
# build robustness — pin via the *_REF vars below if a release ever
# regresses.

set -eu

GTK_REPO="https://github.com/vinceliuice/WhiteSur-gtk-theme.git"
ICON_REPO="https://github.com/vinceliuice/WhiteSur-icon-theme.git"
CURSOR_REPO="https://github.com/vinceliuice/WhiteSur-cursors.git"

GTK_REF="${WHITESUR_GTK_REF:-master}"
ICON_REF="${WHITESUR_ICON_REF:-master}"
CURSOR_REF="${WHITESUR_CURSOR_REF:-master}"

# Accent: our shell uses #7c5cff (violet). WhiteSur's "purple" variant
# is the closest match, so GTK + icon accents stay coherent with the
# Jarvis bar instead of clashing with the default blue.
ACCENT="purple"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "─── WhiteSur GTK theme (${GTK_REF}, accent=${ACCENT}) ───"
git clone --depth 1 --branch "$GTK_REF" "$GTK_REPO" "$work/gtk"
# -d: system dest. -c dark: dark variant only (our base is dark).
# -t purple: accent. --silent-mode: no interactive prompts.
# Only the GTK3 + GTK4 fixed themes — no libadwaita patching (that
# rewrites per-user config, useless in an immutable image build).
"$work/gtk/install.sh" \
    --dest /usr/share/themes \
    --color dark \
    --theme "$ACCENT" \
    --silent-mode

echo "─── WhiteSur icon theme (${ICON_REF}, accent=${ACCENT}) ───"
git clone --depth 1 --branch "$ICON_REF" "$ICON_REPO" "$work/icons"
# -d: system dest. -t purple: one accent variant. We deliberately
# DON'T pass --bold: the bold folder set roughly doubles the install
# (the icon repo is already the heaviest part) and was the likely
# OOM trigger on a memory-constrained WSL build.
"$work/icons/install.sh" \
    --dest /usr/share/icons \
    --theme "$ACCENT"

echo "─── WhiteSur cursors (${CURSOR_REF}) ───"
git clone --depth 1 --branch "$CURSOR_REF" "$CURSOR_REPO" "$work/cursors"
( cd "$work/cursors" && ./install.sh )

echo "─── WhiteSur install complete ───"
