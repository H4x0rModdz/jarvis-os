#!/usr/bin/env bash
# End-to-end test for Lilith's persistent memory (Phase 2a).
#
# Validates the central promise: facts survive a daemon restart.
#   1. Start lilith with a fresh fact DB
#   2. Remember a fact via natural language ("lembrar idioma = pt-br")
#   3. Recall via direct DBus method — works
#   4. Kill lilith
#   5. Start a brand new lilith pointing at the same DB
#   6. Recall the same fact — it's still there
#   7. Forget it
#   8. Recall — empty
#
# No Ollama / no Action Bus required: memory.* tools bypass both.
#
# Run from repository root:
#     bash tools/dev/test-lilith-memory.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
: "${LILITH:=$REPO_ROOT/target/release/jarvis-lilith}"
[[ -x "$LILITH" ]] || { echo "missing $LILITH — run cargo build --release -p jarvis-lilith first"; exit 1; }

# Fresh DB in a temp HOME so we don't clobber the user's real fact store.
TMP_HOME="$(mktemp -d)"
trap 'rm -rf "$TMP_HOME"' EXIT

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

wait_for_gone() {
    local name="$1"
    for _ in {1..50}; do
        if ! dbus-send --session --print-reply=literal \
            --dest=org.freedesktop.DBus /org/freedesktop/DBus \
            org.freedesktop.DBus.NameHasOwner "string:$name" 2>/dev/null \
            | grep -q true; then
            return 0
        fi
        sleep 0.1
    done
    echo "timeout waiting for $name to disappear"
    return 1
}

HOME="$TMP_HOME" dbus-run-session -- bash -c '
    set -e
    export HOME="'"$TMP_HOME"'"

    # ─── First daemon instance ───
    "'"$LILITH"'" >/tmp/lilith-mem-1.log 2>&1 &
    LL_PID=$!

    '"$(declare -f wait_for_service)"'
    '"$(declare -f wait_for_gone)"'
    '"$(declare -f run)"'

    wait_for_service com.jarvis.Lilith
    echo "first lilith up (pid=$LL_PID, HOME=$HOME)"

    run "remember via NL: lembrar idioma = pt-br" \
        dbus-send --session --print-reply --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Command string:"lembrar idioma = pt-br"

    run "remember via direct DBus" \
        dbus-send --session --print-reply --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Remember \
        string:"favorite editor" string:"vscode"

    run "list facts" \
        dbus-send --session --print-reply --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.ListFacts

    run "recall via NL: recall idioma" \
        dbus-send --session --print-reply --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Command string:"recall idioma"

    # ─── Kill first daemon and start a fresh one against the same DB ───
    echo
    echo "─── killing first daemon, starting second on same DB ───"
    kill $LL_PID
    wait_for_gone com.jarvis.Lilith

    "'"$LILITH"'" >/tmp/lilith-mem-2.log 2>&1 &
    LL_PID2=$!
    trap "kill $LL_PID2 2>/dev/null || true" EXIT
    wait_for_service com.jarvis.Lilith
    echo "second lilith up (pid=$LL_PID2)"

    run "recall after restart: idioma" \
        dbus-send --session --print-reply --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Recall string:"idioma"

    run "recall after restart: favorite editor (different casing)" \
        dbus-send --session --print-reply --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Recall string:"FAVORITE editor"

    run "forget via NL: esquecer idioma" \
        dbus-send --session --print-reply --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Command string:"esquecer idioma"

    run "recall after forget: idioma (expect null)" \
        dbus-send --session --print-reply --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Recall string:"idioma"

    echo
    echo "─── lilith log tail (second instance) ───"
    tail -10 /tmp/lilith-mem-2.log
'
