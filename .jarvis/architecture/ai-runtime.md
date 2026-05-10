# Architecture: AI Runtime (Lilith Core)

## Purpose

Defines how Lilith operates as a first-class system daemon within Jarvis OS.

## Process Model

Lilith runs as a user-session daemon:

```
systemd --user
  └── lilith.service
        ├── intent-parser (thread)
        ├── tool-executor (thread)
        ├── memory-manager (thread)
        └── voice-bridge (subprocess)
```

Crash of voice-bridge does not crash Lilith core.
Crash of Lilith surfaces a recoverable error in the UI — desktop remains functional.

## Communication Interfaces

```
UI → Lilith:      DBus  com.jarvis.Lilith.Command(text: string) -> string
Voice → Lilith:   Unix socket (raw transcript stream)
Lilith → JAB:     DBus  com.jarvis.ActionBus.Dispatch(action: json) -> result
Lilith → UI:      DBus  com.jarvis.Lilith.Response signal
```

## Intent Resolution Flow

```
Input (text or voice transcript)
  → Intent Parser (LLM call or rule-based)
  → Structured Intent { intent, params, confidence }
  → Tool Selector
  → Action Bus Dispatch
  → Result → Response formatter
  → UI / TTS output
```

## Model Tiers

| Use Case | Model | Where |
|---|---|---|
| Intent parsing | Local LLM (Ollama) | Preferred |
| Complex reasoning | Claude API | Fallback |
| STT | Faster-Whisper | Local always |
| TTS | Piper | Local always |

Local always wins on privacy. API is opt-in.

## Memory Architecture

```
memory/
  session.db       ← cleared each session
  persistent.db    ← user preferences, saved automations
  system.db        ← app capabilities registry (read-only for Lilith)
```

SQLite for all tiers. No cloud sync unless explicitly opted in.

## Failure Handling

| Failure | Response |
|---|---|
| LLM unreachable | Fall back to rule-based intent matching |
| Action permission denied | Inform user, do not retry |
| Action execution failed | Report clearly with explanation |
| STT silence timeout | Gracefully close listening session |
| Memory DB corrupt | Rebuild from backup, notify user |
