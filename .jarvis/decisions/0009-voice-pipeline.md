# ADR 0009: Voice Pipeline — Whisper STT + Piper TTS, Daemon-Mediated

## Status
Accepted

## Context

The always-present bar at the bottom of the screen only earns its
shape if you can talk to Lilith without picking up a keyboard. Voice
is also the differentiator between Jarvis OS and any other Linux
distribution with Ollama installed — typed-only AI is a tray icon
problem, not an OS problem.

The voice pipeline has two halves:

- **STT** — push-to-talk, capture microphone audio, transcribe to
  text, hand the text to Lilith.Command exactly as if the user had
  typed it.
- **TTS** — when Lilith replies, optionally speak the reply back.

Both must run locally — the project bible's "fully local AI" rule is
not negotiable and there's no cloud STT/TTS that would be welcome here
anyway.

## Decision

A new system module **`system/voice/`** with a Rust daemon
`jarvis-voice` exposing `com.jarvis.Voice` on the session bus. It
mediates microphone access and audio playback, holds the heavy model
state in-process, and exposes a thin DBus surface for the shell.

The actual STT/TTS engines:

- **STT: whisper.cpp** (subprocess of the `whisper-cli` binary
  shipping the `whisper-small` model, ~500 MB). Stays out-of-process
  in V2 — we call it per utterance with a captured WAV. V3 may move
  to a long-running whisper server if cold-start latency becomes an
  issue in practice.
- **TTS: piper** (subprocess of the `piper` binary with a voice model
  like `pt_BR-faber-medium`, ~60 MB). Same shape: invoked per
  Speak() call, output WAV played through `paplay`.
- **Audio capture: rust-cpal** in the daemon process for low-latency
  push-to-talk and clean stop semantics. We could shell out to
  `arecord`, but stop-on-button-release is sharper through a
  library handle than through SIGTERM-ing a child.

## Scope Split

Three commits, each independently useful, none half-built:

| Phase | Deliverable |
|---|---|
| V1 (this commit set) | Daemon skeleton + DBus interface returning `Unavailable` for STT/TTS, a state machine the shell can render, and the mic button on the bar. The shape of the surface is settled before any heavyweight integration. |
| V2 | whisper.cpp wiring + cpal-based microphone capture. Push-to-talk button works end-to-end: hold → speak → release → text goes into Lilith.Command. |
| V3 | piper wiring + auto-speak Lilith replies. Voice models pulled by the Updater (Phase 2) so the ISO doesn't bloat. |

## Reasons

- **Daemon, not in-process in Lilith.** Lilith already does enough
  (Ollama + tools + memory). Adding audio I/O and a 500 MB model
  loaded into the same process couples crashes that should be
  independent — a broken voice subsystem must not take chat down.
- **Separate from the shell.** The shell is a Qt UI process; we don't
  want it owning the microphone or holding a 500 MB whisper context.
  Same reason Permission and Updater are their own daemons.
- **Subprocess engines, not Rust bindings (yet).** `whisper-rs` and
  `piper-rs` exist but their version churn vs the C++ projects is
  ongoing. The subprocess shape is one process boundary — robust,
  upgrade-able independently, and any improvement to the C++ project
  lands at the daemon without a Rust rebuild.
- **Push-to-talk first, hotword later.** Always-listening is a
  different privacy contract and a different UX. Phase 2 ships the
  push-to-talk button; an always-listening hotword (`microphone.listen`
  scope, persistent grant) is Phase 3+ once the rest of the stack
  earns the trust.

## Consequences

- New crate `system/voice/` in the workspace, new binary in the OCI
  image. V1 binary is small (~3 MB stripped). V2 adds whisper-cli +
  model file in `/usr/share/whisper-models/` (~500 MB). V3 adds piper
  + voice model (~60 MB).
- New systemd user unit `jarvis-voice.service`, wired as `Wants=`
  from `jarvis-session.target` (the rest of the session must work
  when voice is offline — see the parallel decision for Updater in
  ADR 0007).
- New Qt bridge `VoiceBridge` in the shell + a mic button component
  on the bar between the clock and Lilith input.
- The `microphone.listen` permission scope (already dangerous in the
  policy) is consumed by `Voice.StartListening`. Lilith pushing
  voice is not the trigger — the user clicking the mic button is —
  so this is a one-time approval on first use, not a per-turn prompt.

## Alternatives Considered

- **Bake STT into Lilith.** Rejected: tight coupling, balloons
  Lilith's process to ~700 MB resident, crashes either way affect the
  other.
- **Use a cloud STT (OpenAI Whisper API, etc.) as default with local
  fallback.** Rejected: bible explicit non-goal. Local is the only
  default — opt-in cloud is a future research topic, not Phase 2.
- **Always-listening hotword from V1.** Rejected: changes the privacy
  contract dramatically (always recording vs. only-when-button-held)
  and we don't yet trust the approval surface enough to make that
  default. Phase 3+.
- **Skip the daemon, capture audio inside the shell.** Rejected: the
  shell already handles 5 DBus surfaces; adding cpal + model loading
  to it muddies the responsibility line we've held since Phase 1.
