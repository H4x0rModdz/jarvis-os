#!/usr/bin/env bash
# Fast dev loop for a RUNNING Jarvis OS VM — no ISO rebuild.
#
# The 30-minute ISO build only matters for packaging / units / base
# image changes. For day-to-day iteration on a Rust daemon or the Qt
# shell, this script rebuilds JUST the thing you changed and pushes
# the fresh binary into a VM that's already installed and booted:
#
#   1. cargo/cmake builds the target locally (or you point it at a
#      prebuilt binary).
#   2. ssh + `bootc usroverlay` makes the VM's /usr writable until its
#      next reboot (bootc is immutable by default).
#   3. scp the binary over, restart the matching service / relaunch
#      the shell.
#
# Round-trip: ~1-2 min (mostly the local compile) vs. ~30 min ISO.
#
# Usage:
#   tools/dev-deploy.sh lilith            # build + deploy jarvis-lilith
#   tools/dev-deploy.sh shell             # build + deploy jarvis-shell
#   tools/dev-deploy.sh shell --qml-only  # skip C++ build, just rsync
#                                         # the qml/ dir + restart (uses
#                                         # JARVIS_QML_PATH on the VM)
#
# Config via env (or edit the defaults):
#   JARVIS_VM_HOST   ssh target            (default: jarvis@127.0.0.1)
#   JARVIS_VM_PORT   ssh port              (default: 2222 — VBox NAT
#                                           forward localhost:2222 → :22)
#
# Requirements on the VM (one-time): sshd enabled, the `jarvis` user
# in sudoers, and `bootc` present (it is on the Fedora bootc base).

set -euo pipefail

VM_HOST="${JARVIS_VM_HOST:-jarvis@127.0.0.1}"
VM_PORT="${JARVIS_VM_PORT:-2222}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

ssh_vm() { ssh -p "$VM_PORT" "$VM_HOST" "$@"; }
scp_vm() { scp -P "$VM_PORT" "$@"; }

target="${1:-}"
shift || true
qml_only=0
for arg in "$@"; do
    [ "$arg" = "--qml-only" ] && qml_only=1
done

if [ -z "$target" ]; then
    echo "usage: $0 <lilith|action-bus|voice|shell|greeter|lock|...> [--qml-only]" >&2
    exit 2
fi

# Map a friendly name to (crate or shell, binary path, restart command).
# Rust daemons are systemd USER services; the shell is launched from
# labwc autostart so we kill + relaunch it instead.
case "$target" in
    lilith|action-bus|voice|settings|notifications|permission|compat|lock|updater)
        kind="rust"
        bin="jarvis-${target}"
        crate="jarvis-${target}"
        ;;
    shell)
        kind="qt"
        bin="jarvis-shell"
        srcdir="shell/jarvis-shell"
        ;;
    greeter)
        kind="qt"
        bin="jarvis-greeter"
        srcdir="shell/jarvis-greeter"
        ;;
    *)
        echo "unknown target: $target" >&2
        exit 2
        ;;
esac

echo "─── target: $target ($kind) ───"

# ── QML-only fast path (Qt targets) ──────────────────────────────────
# No compile. rsync the qml/ dir to the VM + point the binary at it via
# JARVIS_QML_PATH, then relaunch. Sub-second turnaround on QML edits.
if [ "$qml_only" = "1" ]; then
    if [ "$kind" != "qt" ]; then
        echo "--qml-only only applies to shell/greeter" >&2
        exit 2
    fi
    echo "  rsync ${srcdir}/qml → VM /var/home/jarvis/dev-qml"
    ssh_vm "mkdir -p ~/dev-qml/Jarvis/Shell"
    # The on-disk module tree must match the URI (Jarvis.Shell →
    # Jarvis/Shell/). Copy qml/* into that layout + drop a qmldir the
    # engine can read. (For greeter the URI differs; extend as needed.)
    rsync -az -e "ssh -p $VM_PORT" \
        "${REPO_ROOT}/${srcdir}/qml/" \
        "${VM_HOST}:~/dev-qml/Jarvis/Shell/"
    echo "  relaunch ${bin} with JARVIS_QML_PATH"
    ssh_vm "pkill -x ${bin} || true; sleep 0.3; \
            JARVIS_QML_PATH=\$HOME/dev-qml nohup ${bin} >/tmp/${bin}.log 2>&1 &"
    echo "─── done (qml-only) — tail /tmp/${bin}.log on the VM ───"
    exit 0
fi

# ── Build ────────────────────────────────────────────────────────────
# NOTE: build runs HERE. For Qt targets the host needs Qt 6.5+ + the
# project deps; if your host can't (e.g. WSL ships Qt 6.4), run this
# script INSIDE the VM instead — it has the full Fedora 42 toolchain.
built_bin=""
if [ "$kind" = "rust" ]; then
    echo "  cargo build --release -p ${crate}"
    ( cd "$REPO_ROOT" && cargo build --release -p "$crate" )
    built_bin="${REPO_ROOT}/target/release/${bin}"
else
    echo "  cmake build ${srcdir}"
    ( cd "$REPO_ROOT" \
        && cmake -S "$srcdir" -B "/tmp/jarvis-${target}-build" -G Ninja \
                 -DCMAKE_BUILD_TYPE=Release \
        && cmake --build "/tmp/jarvis-${target}-build" -j )
    built_bin="/tmp/jarvis-${target}-build/${bin}"
fi
test -x "$built_bin" || { echo "build produced no binary at $built_bin" >&2; exit 1; }

# ── Deploy ───────────────────────────────────────────────────────────
echo "  bootc usroverlay (make /usr writable until reboot)"
ssh_vm "sudo bootc usroverlay || true"

echo "  scp ${bin} → /usr/bin/${bin}"
scp_vm "$built_bin" "${VM_HOST}:/tmp/${bin}.new"
ssh_vm "sudo install -m 0755 /tmp/${bin}.new /usr/bin/${bin} && rm -f /tmp/${bin}.new"

# ── Restart ──────────────────────────────────────────────────────────
if [ "$kind" = "rust" ]; then
    echo "  systemctl --user restart jarvis-${target}"
    ssh_vm "systemctl --user restart jarvis-${target} || true"
else
    echo "  relaunch ${bin}"
    ssh_vm "pkill -x ${bin} || true; sleep 0.3; nohup ${bin} >/tmp/${bin}.log 2>&1 &"
fi

echo "─── done — ${target} updated on the VM ───"
