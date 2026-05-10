# Lilith — AI Assistant Daemon

## Purpose

Lilith is the conversational AI core of Jarvis OS. It turns natural-language input ("abra o vscode", "minimize this window", "create a folder called Reports") into Action Bus dispatches.

## Boundaries

- Lilith **does not** execute system calls directly. Every effect goes through the Action Bus.
- Lilith **does not** decide permissions — the Action Bus does. Permission denial is surfaced back to the user, never bypassed.
- Lilith **may** keep ephemeral session context. Persistent memory is opt-in and exportable.
- Lilith **must** be functional when no LLM is reachable (rule-based fallback).

## Interfaces

```
DBus  com.jarvis.Lilith
  method Command(text: string) -> string   // JSON response
  method Reset()                            // clear session memory
```

```
HTTP  http://localhost:11434/api/chat       // Ollama (optional)
DBus  com.jarvis.ActionBus.Dispatch         // tool execution
```

## Pipeline

```
text input
  ├─→ rule parser (regex / keyword)     ← always tried first, deterministic
  │     └─ on match → ToolCall { action, params }
  └─→ Ollama /api/chat with tools=[...]  ← fallback if rules miss and host reachable
        └─ on tool_calls → ToolCall(s)

ToolCall → Action Bus Dispatch → response → audit log → reply to caller
```

## Tools

Each Action Bus action is exposed as a Lilith tool with a JSON Schema describing parameters. The schema is sent to the LLM verbatim so it can decide when to invoke each tool. Initial set mirrors the Action Bus exactly (~20 actions): `app.*`, `file.*`, `window.*`, `workspace.*`, `system.*`.

## Memory

v1: session-only, in-memory. Reset on `Reset()` or daemon restart.
v2 (later): SQLite `session.db` and `persistent.db` per the architecture doc.

## Failure modes

| Failure | Behavior |
|---|---|
| Rule miss + Ollama unreachable | Return `"unknown intent"` response, do not block |
| Ollama returns invalid tool call | Log, surface error to user |
| Action Bus denies permission | Surface denial to user, do not retry |
| Action Bus returns error | Surface error verbatim |

## Out of scope (Phase 1)

- Voice (STT/TTS) — separate `voice-bridge` subprocess in a later phase
- Claude API fallback — local-first; API integration deferred
- Multi-turn planning / workflow chains
