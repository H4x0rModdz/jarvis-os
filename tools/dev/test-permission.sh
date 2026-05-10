#!/usr/bin/env bash
# End-to-end test for the Permission System.
#
# Boots permission, action-bus, and lilith under a fresh DBus session.
# Exercises the full chain:
#   1. A dangerous action (file.delete) is denied by default
#   2. Grant lilith the filesystem.delete scope
#   3. Same action now reaches the handler (and fails for the actual fs reason,
#      not permission)
#   4. Revoke
#   5. Same action denied again
#
# No Ollama required — uses the regex rule path.
#
# Run from repository root:
#     bash tools/dev/test-permission.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PERMISSION="$REPO_ROOT/target/release/jarvis-permission"
ACTION_BUS="$REPO_ROOT/target/release/jarvis-action-bus"
LILITH="$REPO_ROOT/target/release/jarvis-lilith"

for b in "$PERMISSION" "$ACTION_BUS" "$LILITH"; do
    [[ -x "$b" ]] || { echo "missing $b — run cargo build --release first"; exit 1; }
done

run() {
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

# Force file.delete to be the action under test: it needs a path arg, and the
# handler will try to call `gio trash <path>` on the target. With the scope
# denied we should never reach the handler; once granted we'll see a real
# filesystem error (since gio is absent in WSL).
TARGET_FILE="/tmp/lilith-perm-test-file.txt"
echo "scratch" > "$TARGET_FILE"

dbus-run-session -- bash -c '
    set -e
    "'"$PERMISSION"'"  >/tmp/jarvis-perm.log         2>&1 &
    P_PID=$!
    "'"$ACTION_BUS"'" >/tmp/jarvis-action-bus.log    2>&1 &
    AB_PID=$!
    "'"$LILITH"'"     >/tmp/jarvis-lilith.log        2>&1 &
    LL_PID=$!
    trap "kill $P_PID $AB_PID $LL_PID 2>/dev/null || true" EXIT

    '"$(declare -f wait_for_service)"'
    wait_for_service com.jarvis.PermissionSystem
    wait_for_service com.jarvis.ActionBus
    wait_for_service com.jarvis.Lilith
    echo "daemons up (permission=$P_PID action-bus=$AB_PID lilith=$LL_PID)"

    '"$(declare -f run)"'

    REQUEST_JSON='"'"'{"action":"file.delete","caller":{"type":"lilith"},"params":{"path":"'"$TARGET_FILE"'","permanent":false},"session_id":"00000000-0000-0000-0000-000000000001","idempotency_key":null}'"'"'

    run "no grant: file.delete should be DENIED" \
        dbus-send --session --print-reply --dest=com.jarvis.ActionBus \
        /com/jarvis/ActionBus com.jarvis.ActionBus.Dispatch "string:$REQUEST_JSON"

    run "grant lilith filesystem.delete" \
        dbus-send --session --print-reply --dest=com.jarvis.PermissionSystem \
        /com/jarvis/PermissionSystem com.jarvis.PermissionSystem.Grant \
        string:lilith string:filesystem.delete boolean:false

    run "list grants" \
        dbus-send --session --print-reply --dest=com.jarvis.PermissionSystem \
        /com/jarvis/PermissionSystem com.jarvis.PermissionSystem.ListGrants

    run "with grant: file.delete should reach handler (and fail for non-permission reason)" \
        dbus-send --session --print-reply --dest=com.jarvis.ActionBus \
        /com/jarvis/ActionBus com.jarvis.ActionBus.Dispatch "string:$REQUEST_JSON"

    run "revoke" \
        dbus-send --session --print-reply --dest=com.jarvis.PermissionSystem \
        /com/jarvis/PermissionSystem com.jarvis.PermissionSystem.Revoke \
        string:lilith string:filesystem.delete

    run "after revoke: DENIED again" \
        dbus-send --session --print-reply --dest=com.jarvis.ActionBus \
        /com/jarvis/ActionBus com.jarvis.ActionBus.Dispatch "string:$REQUEST_JSON"

    echo
    echo "─── permission log tail ───"
    tail -15 /tmp/jarvis-perm.log
'

rm -f "$TARGET_FILE"
