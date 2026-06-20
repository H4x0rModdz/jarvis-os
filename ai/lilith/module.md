# Lilith — AI Assistant Daemon

## Purpose

Lilith is the conversational AI core of LilithOS. It turns natural-
language input ("abra o vscode", "tirar print", "volume 50") into
Action Bus dispatches, then narrates the result back to the user.

## Boundaries

- Lilith **does not** execute system calls directly. Every effect goes
  through the Action Bus.
- Lilith **does not** decide permissions — the Action Bus does, via the
  Permission System. Permission denials are surfaced verbatim to the
  user, never bypassed or retried.
- Lilith **may** keep ephemeral session context and persistent facts.
  Both are local-only and exportable.
- Lilith **must** be functional when no LLM is reachable — the rule-based
  intent parser covers the common phrases without an Ollama round trip.

## Interface

```
DBus  com.jarvis.Lilith  at  /com/jarvis/Lilith

  Command(text: string) -> string   // JSON { reply, action, result, emotion }
                                     // emotion ∈ neutral|happy|concerned drives
                                     // the embodied avatar's face (ADR 0028)
  Recall(key: string)   -> string   // JSON { found, value? }
  Reset()                            // wipe in-memory session ring
```

```
HTTP  http://localhost:11434/api/chat       // Ollama (optional)
DBus  com.jarvis.ActionBus.Dispatch          // tool execution
```

## Pipeline

```
text input
  ├─→ rule parser (intent.rs, regex)             ← always tried first
  │     └─ on match → single ToolCall, one-shot
  └─→ Ollama /api/chat                            ← natural language path
        loop (max 4 steps):
          chat(messages, tools)
            ├─ tool_call(s) → dispatch first → append result → loop
            └─ text only    → final answer

ToolCall → Action Bus Dispatch
        ↓
   memory.* tools handled in-process (no bus round trip)
   everything else → DBus call to com.jarvis.ActionBus

response → audit log per step (~/.jarvis/logs/lilith.log) → reply to caller
```

The chain lets Lilith handle compound requests in one turn — e.g.
"tira um screenshot e abre no editor" issues `screenshot.capture`,
gets back the saved path in the tool result, then issues
`app.open` against that path. Each step gets its own audit entry +
its own session-memory Turn so cross-turn follow-ups still see the
sub-steps.

## Tools

Lilith exposes one tool per Action Bus action, with a JSON Schema
that's sent verbatim to the LLM so it can decide when to call each. The
catalog mirrors the bus (28 actions today: `app.*`, `file.*`,
`window.*`, `workspace.*`, `system.*`, `browser.*`, `clipboard.*`,
`screenshot.*`, `audio.*`) plus three Lilith-internal tools
(`memory.remember`, `memory.recall`, `memory.forget`) that bypass the
bus and write to the local fact store.

## Memory

| Layer | Storage | Lifetime | API |
|---|---|---|---|
| Session ring | RAM, last 32 turns | until daemon restart or `Reset()` | the last 8 turns are flattened into user/assistant messages and prepended to every `ollama.chat()` call so follow-ups resolve against context |
| Fact store | SQLite at `~/.jarvis/lilith/facts.db` | persistent | `memory.remember`, `memory.recall`, `memory.forget` tools |

The Phase 1 design called for split `session.db` / `persistent.db`
files; we collapsed to one SQLite database with a single `facts` table
once it was clear we didn't need durable session memory.

## Test harness

Phase 12 extracted `Ollama` and `BusDispatcher` traits so `LilithService`
fields are `Arc<dyn Ollama>` and `Arc<dyn BusDispatcher>`. The test
module in `main.rs` ships `MockOllama` (scripted replies) and
`MockBus` (recorded dispatches) so the help intent, dispatch
bookkeeping, and the in-process `memory.*` tools are covered without
a live DBus or Ollama instance.

Phase 13 closed the integration-test gap. `signals.rs` defines the
`SignalSink` trait; production wires it as `DbusSignalSink`
(wrapping the zbus `SignalContext`), tests use `RecordingSink` which
captures `(step, payload)` tuples. The chain loop now has scenario
coverage: rule path, help, plain text, single-tool + text wrap-up,
multi-step chain (screenshot → app.open), step-cap hit, Ollama
error fallback.

## Failure modes

| Failure | Behavior |
|---|---|
| Rule miss + Ollama unreachable | Return `"unknown intent"` to the caller; do not block. |
| Ollama returns invalid tool call (unknown action / bad shape) | Log, surface as an error reply, no dispatch. |
| Action Bus denies permission | Reply forwards the denial reason; no automatic retry. |
| Action Bus returns error from a handler | Reply forwards the error message verbatim. |
| Tool schema mismatch (LLM hallucinates extra field) | Pass through — handlers ignore unknown params. |

## Out of scope (Phase 1)

- Voice (STT/TTS) — separate Phase 2 module with its own daemon.
- Claude API or any cloud LLM fallback — local-first by design.
- Multi-turn planning / workflow chains — single tool call per turn for now.
- Cross-session shared memory between users.
