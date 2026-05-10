#!/usr/bin/env bash
# End-to-end test for Lilith + Action Bus inside a dedicated DBus session.
#
# Boots both daemons under `dbus-run-session`, waits for them to register
# their service names, fires a handful of rule-based commands at Lilith,
# then tears everything down. No external services required (no Ollama,
# no display, no compositor).
#
# Run from the repository root:
#     bash tools/dev/test-lilith.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ACTION_BUS="$REPO_ROOT/target/release/jarvis-action-bus"
LILITH="$REPO_ROOT/target/release/jarvis-lilith"

[[ -x "$ACTION_BUS" ]] || { echo "missing $ACTION_BUS — run cargo build --release first"; exit 1; }
[[ -x "$LILITH" ]]     || { echo "missing $LILITH — run cargo build --release first"; exit 1; }

run() {
    # Pretty header before each dbus-send call so the transcript is readable.
    echo
    echo "─── $1 ───"
    shift
    "$@"
}

wait_for_service() {
    local name="$1"
    for _ in {1..50}; do
        if dbus-send --session --print-reply=literal \
            --dest=org.freedesktop.DBus /org/freedesktop/DBus \
            org.freedesktop.DBus.NameHasOwner "string:$name" 2>/dev/null \
            | grep -q true; then
            return 0
        fi
        sleep 0.1
    done
    echo "timeout waiting for $name"
    return 1
}

dbus-run-session -- bash -c '
    set -e

    "'"$ACTION_BUS"'" >/tmp/lilith-test-action-bus.log 2>&1 &
    AB_PID=$!
    "'"$LILITH"'" >/tmp/lilith-test-lilith.log 2>&1 &
    LL_PID=$!
    trap "kill $AB_PID $LL_PID 2>/dev/null || true" EXIT

    '"$(declare -f wait_for_service)"'
    wait_for_service com.jarvis.ActionBus
    wait_for_service com.jarvis.Lilith
    echo "both daemons up (action-bus=$AB_PID lilith=$LL_PID)"

    '"$(declare -f run)"'

    run "list registered actions" dbus-send --session --print-reply --dest=com.jarvis.ActionBus \
        /com/jarvis/ActionBus com.jarvis.ActionBus.ListActions

    run "rule: notify (pt)" dbus-send --session --print-reply --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Command string:"notify: Lilith está viva"

    run "rule: open app (pt)" dbus-send --session --print-reply --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Command string:"abrir firefox"

    run "rule: window minimize (en)" dbus-send --session --print-reply --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Command string:"minimize window 42"

    run "unknown intent" dbus-send --session --print-reply --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Command string:"what is the weather"

    echo
    echo "─── action-bus log tail ───"
    tail -20 /tmp/lilith-test-action-bus.log

    echo
    echo "─── lilith log tail ───"
    tail -20 /tmp/lilith-test-lilith.log
'
