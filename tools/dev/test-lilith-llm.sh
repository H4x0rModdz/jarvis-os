#!/usr/bin/env bash
# Tests the Ollama-fallback path: phrases that don't match any regex rule
# and must be interpreted by the LLM to pick a tool call.
#
# Prerequisites:
#   - ollama serve listening on http://localhost:11434
#   - model pulled: ollama pull qwen3:4b   (or set LILITH_MODEL=...)
#
# Run from repository root:
#     bash tools/dev/test-lilith-llm.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ACTION_BUS="$REPO_ROOT/target/release/jarvis-action-bus"
LILITH="$REPO_ROOT/target/release/jarvis-lilith"

[[ -x "$ACTION_BUS" ]] || { echo "missing $ACTION_BUS"; exit 1; }
[[ -x "$LILITH"     ]] || { echo "missing $LILITH"; exit 1; }

curl -sf http://localhost:11434/api/version >/dev/null \
    || { echo "ollama not reachable on :11434 — run 'ollama serve' first"; exit 1; }

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

dbus-run-session -- bash -c '
    set -e

    "'"$ACTION_BUS"'" >/tmp/lilith-llm-action-bus.log 2>&1 &
    AB_PID=$!
    "'"$LILITH"'" >/tmp/lilith-llm-lilith.log 2>&1 &
    LL_PID=$!
    trap "kill $AB_PID $LL_PID 2>/dev/null || true" EXIT

    '"$(declare -f wait_for_service)"'
    wait_for_service com.jarvis.ActionBus
    wait_for_service com.jarvis.Lilith
    echo "both daemons up (action-bus=$AB_PID lilith=$LL_PID)"

    '"$(declare -f run)"'

    # Each phrase deliberately avoids the regex patterns so the LLM has to do the work.
    # Warm up the model (first call after `ollama serve` start loads weights into RAM).
    echo
    echo "─── warming up qwen3:4b ───"
    curl -sf -X POST http://localhost:11434/api/generate \
        -d "{\"model\":\"qwen3:4b\",\"prompt\":\"hi\",\"stream\":false}" >/dev/null \
        && echo "warm-up ok"

    DBUS_TIMEOUT=120000

    run "llm: please launch firefox for me" \
        dbus-send --session --print-reply --reply-timeout=$DBUS_TIMEOUT --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Command \
        string:"please launch the firefox browser for me"

    run "llm: i need a notification saying coffee is ready" \
        dbus-send --session --print-reply --reply-timeout=$DBUS_TIMEOUT --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Command \
        string:"send a notification that says coffee is ready"

    run "llm: small talk (no tool expected)" \
        dbus-send --session --print-reply --reply-timeout=$DBUS_TIMEOUT --dest=com.jarvis.Lilith \
        /com/jarvis/Lilith com.jarvis.Lilith.Command \
        string:"hi, who are you?"

    echo
    echo "─── lilith log tail ───"
    tail -30 /tmp/lilith-llm-lilith.log
'
