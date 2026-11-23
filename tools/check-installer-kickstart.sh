#!/usr/bin/env bash
# tools/check-installer-kickstart.sh — the installer must never pick a disk.
#
# Guards against the bug where the ISO wiped every disk in the machine: an
# unattended kickstart with `clearpart --all` (see iso/build.toml).
#
#   tools/check-installer-kickstart.sh                 check iso/build.toml
#   tools/check-installer-kickstart.sh <manifest.json> also check a manifest
#                                                      produced by
#                                                      bootc-image-builder
#
# Exit 0 = safe, 1 = a storage command was found (or the config has no
# kickstart of ours, which makes the builder generate an unattended one).
set -euo pipefail

CONFIG=iso/build.toml
MANIFEST="${1:-}"
FAIL=0

[ -f "$CONFIG" ] || { echo "run from the repo root ($CONFIG not found)" >&2; exit 1; }

# Kickstart commands that select, wipe or partition disks.
STORAGE='clearpart|zerombr|autopart|ignoredisk|part|partition|logvol|volgroup|raid|btrfs|reqpart'

# 1. Our config must carry its own kickstart — without one the builder writes
#    an unattended kickstart that clears all disks.
if ! grep -q '^\[customizations\.installer\.kickstart\]' "$CONFIG"; then
    echo "FAIL: $CONFIG has no [customizations.installer.kickstart]." >&2
    echo "      bootc-image-builder would generate an unattended install that wipes all disks." >&2
    FAIL=1
fi
# A user/group customization and a custom kickstart are mutually exclusive, and
# a config with only users is exactly what triggers the unattended install.
if grep -q '^\[\[customizations\.\(user\|group\)\]\]' "$CONFIG"; then
    echo "FAIL: $CONFIG uses [[customizations.user|group]] — put accounts in the kickstart instead." >&2
    FAIL=1
fi

# 2. No storage command inside the kickstart text (comment lines don't count).
KS=$(awk '/^contents = """/{inks=1; next} inks && /^"""/{inks=0} inks' "$CONFIG")
BAD=$(printf '%s\n' "$KS" | grep -vE '^[[:space:]]*#' | grep -nE "^[[:space:]]*($STORAGE)([[:space:]]|$)" || true)
if [ -n "$BAD" ]; then
    echo "FAIL: storage command(s) in the installer kickstart ($CONFIG):" >&2
    printf '%s\n' "$BAD" | sed 's/^/      /' >&2
    FAIL=1
fi
if printf '%s\n' "$KS" | grep -vE '^[[:space:]]*#' | grep -qE '^[[:space:]]*(reboot|poweroff|shutdown|halt)([[:space:]]|$)'; then
    echo "FAIL: the installer kickstart reboots/powers off on its own — an install must end on a screen the user reads." >&2
    FAIL=1
fi

# 3. What the builder actually generated. A manifest is JSON; the kickstart
#    stage carries the generated commands as option keys.
if [ -n "$MANIFEST" ]; then
    [ -f "$MANIFEST" ] || { echo "manifest not found: $MANIFEST" >&2; exit 1; }
    if grep -qE '"(clearpart|zerombr|autopart|ignoredisk)"[[:space:]]*:' "$MANIFEST"; then
        echo "FAIL: the generated ISO manifest contains a storage command:" >&2
        grep -oE '"(clearpart|zerombr|autopart|ignoredisk)"[[:space:]]*:[^,}]{0,60}' "$MANIFEST" | sort -u | sed 's/^/      /' >&2
        FAIL=1
    fi
    if grep -qE 'clearpart|autopart --' "$MANIFEST"; then
        # Also catches commands written as raw kickstart text.
        if ! grep -qE '"(clearpart|autopart)"[[:space:]]*:' "$MANIFEST"; then
            echo "FAIL: the generated ISO manifest mentions clearpart/autopart in raw kickstart text." >&2
            FAIL=1
        fi
    fi
fi

if [ "$FAIL" = 0 ]; then
    echo "installer kickstart: OK — no storage commands; the user chooses the disk."
fi
exit "$FAIL"
