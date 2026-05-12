# Jarvis Voice

## Status

**Phase V3 — STT + TTS shipped.** Push-to-talk via whisper.cpp;
auto-speak Lilith replies via piper. The full voice loop is live in
the ISO. See [ADR 0009](../../.jarvis/decisions/0009-voice-pipeline.md).

## Purpose

Mediates microphone capture and audio playback for the OS. Owns the
heavy STT and TTS model state so the shell stays a thin Qt process and
Lilith stays a chat-and-tools daemon — both can crash independently of
the voice stack without taking the other down.

## Boundaries

- Voice **does not** dispatch Action Bus actions. After a successful
  transcription the daemon emits `TranscriptionFinal(text)`; the
  shell (or any subscriber) is the one that pipes the text into
  `com.jarvis.Lilith.Command`. This keeps Voice strictly a transport
  for sound ↔ text.
- Voice **does not** decide permissions. `StartListening` is gated by
  the Action Bus's `microphone.listen` scope when called via the
  bus's bridge; direct DBus callers are trusted (DBus identity is the
  OS bus's job).
- Voice **does not** persist transcripts. Audio buffers live for the
  duration of a press-to-talk; nothing is written to disk. Lilith's
  audit log records what got sent to `Command`, not the raw voice.

## Interface

```
DBus  com.jarvis.Voice  at  /com/jarvis/Voice

  StartListening() -> string   // JSON { started: bool, reason?: string }
       └─ begins capturing from the default microphone. Returns
          immediately. Transcription happens after StopListening.

  StopListening() -> string    // JSON { stopped: bool }
       └─ closes the recording, runs STT, emits TranscriptionFinal
          (or TranscriptionFailed) when ready.

  Cancel() -> string           // JSON { cancelled: bool }
       └─ aborts whatever is in flight (recording / processing /
          speaking). State returns to "idle".

  Speak(text: string) -> string   // JSON { spoken: bool, reason?: string }
       └─ enqueue TTS. Blocks until playback finishes (~200 ms - 5 s).

  GetState() -> string         // JSON { state: "idle"|"listening"|
                                          "processing"|"speaking" }

  signal StateChanged(state: string)
       └─ fires every time the state machine moves.

  signal TranscriptionFinal(text: string)
       └─ fires once per successful StopListening cycle.

  signal TranscriptionFailed(reason: string)
       └─ fires when STT errors out (no audio, model failure, …).
```

## State Machine

```
            ┌──────────────── Cancel ──────────────┐
            │                                       │
   idle ───StartListening──> listening ───StopListening──> processing
    ^                                                              │
    │                                                       Speak  │
    │                                                              │
    └────── TranscriptionFinal / TranscriptionFailed ──────────────┘
                                       │
                                  Speak(text)
                                       ▼
                                  speaking ──(playback done)──> idle
```

`StartListening` from any state other than `idle` returns
`{ started: false, reason: "busy" }`. Same for `Speak`.

## Implementation Phases

| Phase | Microphone | STT | TTS |
|---|---|---|---|
| V1 | — | `Unavailable` | `Unavailable` |
| V2 | cpal | whisper.cpp subprocess | `Unavailable` |
| V3 (current) | cpal | whisper.cpp subprocess | piper subprocess + paplay |

## Failure Modes

| Failure | Behavior |
|---|---|
| Microphone busy / not granted | `StartListening` returns `started: false`, the shell shows the mic button in a "no permission" state. |
| STT model missing on disk | `TranscriptionFailed("model not found")`. The updater is expected to fetch it; V2 ships with the model baked, V3 may pull on first use. |
| TTS voice model missing | `Speak` returns `spoken: false, reason: "voice model not found"`. Reply still shows in the popup, just no audio. |
| Daemon offline | The shell's `VoiceBridge` shows the mic icon as disabled with a tooltip; Lilith chat keeps working in text-only mode. |
