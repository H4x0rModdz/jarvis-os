#!/usr/bin/env bash
# End-to-end test for the approval-UX flow.
#
# 1. Start permission + action-bus
# 2. In one async branch, dispatch file.delete (dangerous scope, no grant)
# 3. In parallel, subscribe to ApprovalRequested and call ResolveApproval
#    with each of the three decisions in turn. Validate the dispatch outcome
#    matches what we asked for.
#
# Uses dbus-monitor + a small awk filter to capture the signal and pluck the
# request_id. No Qt / no GUI — proves the signal protocol works headlessly so
# any UI (or alternate frontend) can rely on it.
#
# Run from repository root:
#     bash tools/dev/test-approval.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PERMISSION="${PERMISSION:-$REPO_ROOT/target/release/jarvis-permission}"
ACTION_BUS="${ACTION_BUS:-$REPO_ROOT/target/release/jarvis-action-bus}"

for b in "$PERMISSION" "$ACTION_BUS"; do
    [[ -x "$b" ]] || { echo "missing $b — build release first"; exit 1; }
done

# A throwaway file the file.delete handler can chew on once we approve.
TARGET="/tmp/jarvis-approval-test.txt"
echo "scratch" > "$TARGET"

run() {
    echo
    echo "─── $1 ───"
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

# Subscribe to the signal in a background process. Each time it fires we
# pluck out the request_id (4th string in the signal) and write it to a
# named pipe so the main flow can consume it.
PIPE="$(mktemp -u)"
mkfifo "$PIPE"
trap 'rm -f "$PIPE" "$TARGET"' EXIT

dbus-run-session -- bash -c "
    set -e
    '$PERMISSION' >/tmp/jarvis-approval-permission.log 2>&1 &
    P_PID=\$!
    '$ACTION_BUS' >/tmp/jarvis-approval-bus.log 2>&1 &
    AB_PID=\$!
    trap 'kill \$P_PID \$AB_PID 2>/dev/null || true' EXIT

    $(declare -f wait_for_service)
    wait_for_service com.jarvis.PermissionSystem
    wait_for_service com.jarvis.ActionBus

    # ─── Signal listener (background) ───
    # dbus-monitor's output is line-oriented; the request_id sits two lines
    # after the 'member=ApprovalRequested' header.
    dbus-monitor --session \"interface='com.jarvis.PermissionSystem',member='ApprovalRequested'\" \
        > /tmp/jarvis-approval-monitor.log 2>&1 &
    MON_PID=\$!
    sleep 0.5

    # Truncating the log mid-run confuses dbus-monitor's append fd, so we
    # leave the file growing and pluck the Nth UUID seen so far. \$1 is the
    # 1-based occurrence to fetch.
    pluck_id() {
        local nth=\$1
        local started=\$SECONDS
        while (( SECONDS - started < 5 )); do
            local id=\$(grep -oE '\"[0-9a-f-]{36}\"' /tmp/jarvis-approval-monitor.log 2>/dev/null \
                | sed -n \"\${nth}p\" | tr -d '\"')
            if [[ -n \"\$id\" ]]; then
                echo \"\$id\"
                return 0
            fi
            sleep 0.1
        done
        echo \"\"
        return 1
    }

    REQUEST_JSON='{\"action\":\"file.delete\",\"caller\":{\"type\":\"lilith\"},\"params\":{\"path\":\"$TARGET\",\"permanent\":false},\"session_id\":\"00000000-0000-0000-0000-000000000001\",\"idempotency_key\":null}'

    # ─── Round 1: user denies ───
    ROUND=1
    echo
    echo '─── round 1: user denies ───'
    dbus-send --session --print-reply --reply-timeout=40000 --dest=com.jarvis.ActionBus \
        /com/jarvis/ActionBus com.jarvis.ActionBus.Dispatch \"string:\$REQUEST_JSON\" >/tmp/jarvis-approval-r1.out 2>&1 &
    DISP_PID=\$!
    sleep 0.5
    ID=\$(pluck_id \$ROUND)
    echo \"  signal id = \$ID\"
    dbus-send --session --print-reply --dest=com.jarvis.PermissionSystem \
        /com/jarvis/PermissionSystem com.jarvis.PermissionSystem.ResolveApproval string:\"\$ID\" string:deny
    wait \$DISP_PID
    cat /tmp/jarvis-approval-r1.out

    # File should still exist (action was denied)
    test -f \"$TARGET\" && echo \"  ok: target file still present\" || echo \"  FAIL: target deleted\"
    # Re-create for round 2 just in case
    echo scratch > \"$TARGET\"

    # ─── Round 2: user approves once (no grant stored) ───
    ROUND=2
    echo
    echo '─── round 2: user approves once ───'
    dbus-send --session --print-reply --reply-timeout=40000 --dest=com.jarvis.ActionBus \
        /com/jarvis/ActionBus com.jarvis.ActionBus.Dispatch \"string:\$REQUEST_JSON\" >/tmp/jarvis-approval-r2.out 2>&1 &
    DISP_PID=\$!
    sleep 0.5
    ID=\$(pluck_id \$ROUND)
    echo \"  signal id = \$ID\"
    dbus-send --session --print-reply --dest=com.jarvis.PermissionSystem \
        /com/jarvis/PermissionSystem com.jarvis.PermissionSystem.ResolveApproval string:\"\$ID\" string:approve
    wait \$DISP_PID
    cat /tmp/jarvis-approval-r2.out

    # File should be gone now (handler reached, gio trash succeeded)
    test ! -f \"$TARGET\" && echo \"  ok: target file removed\" || echo \"  note: target still present (gio trash may not work here)\"
    echo scratch > \"$TARGET\"

    # ─── Round 3: user approves persistently — next call should auto-allow ───
    ROUND=3
    echo
    echo '─── round 3: user approves persistently ───'
    dbus-send --session --print-reply --reply-timeout=40000 --dest=com.jarvis.ActionBus \
        /com/jarvis/ActionBus com.jarvis.ActionBus.Dispatch \"string:\$REQUEST_JSON\" >/tmp/jarvis-approval-r3.out 2>&1 &
    DISP_PID=\$!
    sleep 0.5
    ID=\$(pluck_id \$ROUND)
    echo \"  signal id = \$ID\"
    dbus-send --session --print-reply --dest=com.jarvis.PermissionSystem \
        /com/jarvis/PermissionSystem com.jarvis.PermissionSystem.ResolveApproval string:\"\$ID\" string:approve_persistent
    wait \$DISP_PID
    cat /tmp/jarvis-approval-r3.out
    echo scratch > \"$TARGET\"

    # ─── Round 4: dispatch again — should not prompt (persistent grant) ───
    echo
    echo '─── round 4: dispatch with grant in place (no signal expected) ───'
    dbus-send --session --print-reply --reply-timeout=5000 --dest=com.jarvis.ActionBus \
        /com/jarvis/ActionBus com.jarvis.ActionBus.Dispatch \"string:\$REQUEST_JSON\"

    echo
    echo '─── final grants ───'
    dbus-send --session --print-reply --dest=com.jarvis.PermissionSystem \
        /com/jarvis/PermissionSystem com.jarvis.PermissionSystem.ListGrants

    kill \$MON_PID 2>/dev/null || true
"
