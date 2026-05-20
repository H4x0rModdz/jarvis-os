# Jarvis Voice

## Status

**Phase V4 — STT + TTS + hotword.** Push-to-talk via whisper.cpp;
auto-speak Lilith replies via piper; continuous wake-word listening
for "oi lilith" via a separate Whisper sliding window. The full
voice loop is live in the ISO. See
[ADR 0009](../../.jarvis/decisions/0009-voice-pipeline.md) for the
STT/TTS scope and
[ADR 0015](../../.jarvis/decisions/0015-hotword.md) for hotword.

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

  StartHotword() -> string     // JSON { enabled: bool, reason?: string }
       └─ Begin continuous wake-word listening. Runs an independent
          cpal stream alongside push-to-talk; on PipeWire (Fedora
          default) both share the source cleanly.

  StopHotword() -> string      // JSON { enabled: false }
       └─ Disengage the hotword actor. Idempotent.

  GetHotwordEnabled() -> bool

  signal StateChanged(state: string)
       └─ fires every time the state machine moves.

  signal TranscriptionFinal(text: string)
       └─ fires once per successful StopListening cycle.

  signal TranscriptionFailed(reason: string)
       └─ fires when STT errors out (no audio, model failure, …).

  signal HotwordDetected(text: string)
       └─ fires when the sliding-window transcript contains a
          wake-word substring. `text` is the full transcript; the
          shell strips the wake-word and dispatches the remainder
          (or pops the mic when the remainder is empty).

  EnrollVoiceprint(user: string, seconds: u32) -> string  // JSON
       └─ { ok: bool, user, frames?, reason? }
          Captures `seconds` (clamped 1..=10) of audio, computes the
          V1 temporal log-RMS feature vector, stores it.

  VerifyVoiceprint(user: string) -> string  // JSON
       └─ { ok: bool, score: f32, threshold: f32, reason? }
          Captures ~2 s, compares against the stored print via cosine
          similarity. `score >= threshold` (0.85 in V1) sets ok=true.

  ListEnrolled() -> string  // JSON
       └─ { users: [{ user, enrolled_at }, …] }

  DeleteVoiceprint(user: string) -> string  // JSON
       └─ { deleted: bool }
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

| Phase | Microphone | STT | TTS | Hotword | Voiceprint |
|---|---|---|---|---|---|
| V1 | — | `Unavailable` | `Unavailable` | — | — |
| V2 | cpal | whisper.cpp subprocess | `Unavailable` | — | — |
| V3 | cpal | whisper.cpp subprocess | piper subprocess + paplay | — | — |
| V4 | cpal | whisper.cpp subprocess | piper subprocess + paplay | sliding-window Whisper, separate cpal stream | log-RMS temporal envelope + cosine (scaffold; ADR 0018) |
| V5 | unchanged | unchanged | unchanged | sliding-window Whisper (3 s window, 2 s tick, RMS VAD) | MFCC + DTW (classical biometric pipeline; ADR 0018 amended) |
| V6 (current) | unchanged | unchanged | unchanged | 1.5 s window, 1.5 s tick, RMS+ZCR VAD (~½ V5 CPU, ~1.5 s latency) | unchanged from V5 |
| V7 (next) | unchanged | unchanged | unchanged | openWakeWord ONNX (<300 ms latency, ~5 % CPU) | x-vector / d-vector embeddings (anti-spoofing strength) |

## Test harness

Phase 14 extracted the `Stt` trait (real impl: `stt::WhisperCli`)
so tests can swap in a scripted `MockStt`. `run_stt` is split
into a capture-bound front half and a `transcribe_samples` back
half — the latter writes the WAV, calls the trait, and is the
piece tests cover.

Coverage today: WAV write + Stt dispatch happy path, empty-samples
short-circuit, error propagation, state-enum round-trip,
voiceprint pure-function tests (Phase 5/6). The capture + hotword
actors stay un-mocked — they need their own `AudioCapture` trait,
which is the natural next refactor.

## Failure Modes

| Failure | Behavior |
|---|---|
| Microphone busy / not granted | `StartListening` returns `started: false`, the shell shows the mic button in a "no permission" state. |
| STT model missing on disk | `TranscriptionFailed("model not found")`. The updater is expected to fetch it; V2 ships with the model baked, V3 may pull on first use. |
| TTS voice model missing | `Speak` returns `spoken: false, reason: "voice model not found"`. Reply still shows in the popup, just no audio. |
| Daemon offline | The shell's `VoiceBridge` shows the mic icon as disabled with a tooltip; Lilith chat keeps working in text-only mode. |
| Hotword false-fire (random speech matches "oi lilith") | The shell dispatches whatever remainder follows the wake-word to Lilith; if there's no remainder, the mic pops and waits for a command. Worst case: a stray cycle that ends in "no command found". |
| Hotword can't open the mic (busy by push-to-talk, etc.) | `StartHotword` returns `enabled: false` with the underlying cpal error. Setting persists; daemon retries on the next StartHotword call. |
