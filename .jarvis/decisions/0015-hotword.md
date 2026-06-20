# ADR 0015: Hotword Detection — Whisper Sliding Window, Same Daemon

## Status
Accepted

## Context

The push-to-talk loop (ADR 0009) works but doesn't justify the
always-present bar. The whole point of "Lilith lives in the bar" is
that the user shouldn't have to *reach* for her. Saying "oi lilith"
should be enough. That's what makes the OS feel AI-native instead of
"GNOME with an LLM tab open."

Hotword detection has the same broad design space as STT itself.
Reasonable options:

1. **Dedicated hotword engine** — Porcupine, Snowboy (dead), openWakeWord.
   Real-time, low CPU, hand-trained per phrase.
2. **Whisper sliding window** — reuse the model already shipping in
   the ISO, transcribe a small audio window every N seconds, string-
   match against wake phrases.
3. **VAD + on-demand STT** — pure energy detector, run Whisper only
   when something speech-like crosses threshold.

## Decision

Option 2 — **Whisper sliding window in the existing voice daemon.**

- No new model files in the ISO (Whisper base is already there).
- One process owns the mic. No coordination between a hotword
  daemon and an STT daemon over who's holding cpal.
- We accept higher CPU cost (~10–25% of one core when enabled) in
  exchange for not shipping a second ML stack.
- Wake-word match is loose-substring against the lowercased
  transcript: `"oi lilith"`, `"ei lilith"`, `"olá lilith"`,
  `"hey lilith"`. Whisper's Portuguese head sometimes hears
  "lilith" as "lilit" or "lilith" — the looseness compensates.
- A trivial RMS energy threshold skips silence windows so we don't
  burn whisper-cli on pure noise floor.

## Architecture

```
                ┌──────────────────────────────────────────┐
                │       jarvis-voice (per-user daemon)     │
                │                                          │
                │  ┌────────────────────────────────────┐  │
   cpal stream  │  │ HotwordActor: sliding window       │  │
   ──────────►  │  │  - 16 kHz mono ring buffer (4 s)   │  │
                │  │  - tick every 2 s                  │  │
                │  │  - whisper-cli on the latest 3 s   │  │
                │  │  - substring match → fire signal   │  │
                │  └────────────────────────────────────┘  │
                │                                          │
                │       DBus: HotwordDetected(text)        │
                └────────────────────────────┬─────────────┘
                                             ▼
                                    jarvis-shell binds,
                                    invokes the same flow
                                    the mic button does
                                    (StartListening, etc.)
```

The hotword actor lives on its own thread (cpal's `!Send`
constraint, same pattern as `capture::CaptureActor`). It owns its
own cpal stream — sharing one stream between hotword and explicit
push-to-talk caused too much state tangling in the prototype, and
PipeWire on Fedora multi-routes the default source without a fight.

## Default Posture

**Off by default.** Continuous mic listening is a deliberate
choice, not a surprise. The shell's settings panel exposes a
toggle; `com.jarvis.Voice.StartHotword()` /
`com.jarvis.Voice.StopHotword()` are the programmatic surface. The
setting persists via `com.jarvis.Settings` under
`voice.hotword.enabled`.

## Consequences

**Good:**
- One Whisper, one daemon, one mic surface.
- Privacy story is honest — audio never leaves the device, hotword
  is opt-in, and even when on the daemon only emits transcripts
  that contain the wake phrase.
- Looser substring matching survives ASR jitter without false-firing
  on every "ali" or "ele".

**Bad:**
- 10–25% of one core when enabled. Acceptable on the desktop class
  we target; not viable on a battery-conscious laptop without a V3
  that swaps to openWakeWord or Porcupine.
- 1.5-second tick latency between phrase end and HotwordDetected
  (V1 was 2 s). Still slow vs. dedicated engines that fire in
  under 300 ms.

V3 will probably move to a tiny dedicated wake-word model
(openWakeWord ships ONNX files in the 1–3 MB range) but the
substring-on-Whisper path is fine to ship today.

## V2 update (Phase 6)

Same engine (substring-on-Whisper), tightened loop:

- Window cut from 3 s to 1.5 s. Whisper still has enough context for
  "oi lilith" (two syllables, ~400 ms) and the surrounding silence;
  the ~50 % cut in input length is roughly a ~50 % cut in
  per-tick whisper-cli cost.
- Tick interval cut from 2 s to 1.5 s — matches the window so we
  cover audio continuously, and the worst-case detection latency
  drops from 4 s (tick + window) to 1.5 s (tick = window).
- Ring buffer cut to 3 s (was 4 s) — needed window + margin only.
- VAD: added a zero-crossing-rate ceiling alongside the existing
  RMS energy gate. ZCR > 0.40 with high RMS is the classic
  signature of broadband noise (fricatives at the wrong volume,
  keyboard tapping, AC hum, cooler ramping). Rejected before
  whisper-cli runs.

Net effect: CPU usage roughly halved (window halved, plus VAD
filters out the ~30 % of noise-floor windows that V1 still
transcribed); latency drops from 4 s worst-case to ~1.5 s.

V3 still planned as the openWakeWord swap.

## Alternatives Considered

- **Snowboy** — abandoned upstream; we don't link to a dead project.
- **Porcupine** — proprietary, free tier requires a paid key for
  redistribution. Not compatible with the LilithOS license direction.
- **openWakeWord** — viable; deferred to V2 once the substring-on-
  Whisper path has user feedback.
