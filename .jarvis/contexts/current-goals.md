# Current Goals

> Update this file as goals evolve. This is the active work context.

## Current Phase

**Between phases — Phase 10 closed.** The voice + biometric + AI
loops are end-to-end. Picking the next arc is open; see "Phase 11
candidates" below.

## Decisions Locked

- **Base OS:** Fedora Atomic, OCI image model (ADR 0001, 0005)
- **UI:** Qt6/QML + Wayland (ADR 0002, 0003, 0006)
- **Core IPC:** Jarvis Action Bus via DBus (ADR 0004)
- **Compositor (now):** labwc as a placeholder until the Smithay
  compositor matures. ADR 0006 decision unchanged; the scaffold
  builds (ADR 0006 amended in Phase 4).
- **First-boot UX:** dedicated updater daemon + splash (ADR 0007).
- **App install:** Flatpak/Flathub, `--user` (ADR 0009 — Phase 3).
- **Notifications:** Jarvis owns `org.freedesktop.Notifications`
  (ADR 0010). V3 persists to SQLite.
- **SDK contract:** manifest discovery at well-known paths (ADR 0011).
- **Greeter:** custom Qt UI, three modes (ADR 0012).
- **Compat:** Wine per-app prefixes + Proton-GE direct (ADRs 0013, 0017).
- **Lock:** wlr-layer-shell Overlay + dual PAM stack (ADRs 0014, 0020).
- **Hotword:** sliding-window Whisper (ADR 0015). V2 tightened the loop.
- **Biometric PAM:** custom module + helper binary (ADRs 0016, 0019).
- **Voiceprint matcher:** MFCC + DTW (ADR 0018 — V1 scaffold + V2
  matcher amendment).

## Phases 1–10 outcomes (closed)

| Phase | Headline |
|---|---|
| 1 | Action Bus + Permission + Lilith daemon + Qt shell + bootable ISO |
| 2 | Voice STT/TTS, settings, OS upgrades, 28 actions, launcher polish |
| 3 | Wine compat, Flatpak, SDK, greeter (3 modes), lock V1, notifications V2 + polish (Flatpaks in launcher, drawer dismiss/clear, auto-lock) |
| 4 | Anime avatar slot, pam-jarvis scaffold, Proton-GE compat, Smithay compositor scaffold (opt-in), glassmorphism V1 |
| 5 | `compat.install_proton` + progress, idle timeout configurable, notifications SQLite, compat lifecycle, voiceprint V2 scaffold |
| 6 | MFCC+DTW voiceprint matcher, pam-jarvis V2 (helper binary), hotword V2 (VAD + tighter window) |
| 7 | `jarvis-voiceprint-ctl`, SettingsPanel biometric section, first shipping PAM wiring |
| 8 | Dual PAM stack, `Lock.VerifyVoice()`, voice-unlock pill in lock window |
| 9 | Lilith multi-turn history, multi-step tool chaining, pt-BR system prompt |
| 10 | Lilith streaming responses + chain-step state in the shell |

## End-to-end loops that work today

- **Type + dispatch.** Type into the bar → rule-based intent parses
  OR Ollama tool-call → Action Bus → handler → audit log → reply.
- **Voice command.** Push-to-talk mic → Whisper STT →
  TranscriptionFinal → dispatch through the same path as typed.
- **Hotword wake.** "oi lilith" → sliding-window match → mic pops
  for the command body OR direct dispatch if a command followed
  the wake phrase.
- **Multi-step chain.** "tira um print e abre no editor" → loop of
  (chat → tool → result → chat → …) up to 4 steps; streaming text
  + chain-step indicator visible in the bar input as it runs.
- **Voice enrollment + unlock.** Settings → "Registrar minha voz"
  → 3 s capture → MFCC stored. Lock screen voice pill → 2 s
  capture → MFCC+DTW score → unlock or fallback to typed password.
- **App install.** "instala o gimp" → Lilith → `app.install`
  Flatpak handler → toast on completion → GIMP shows up in the
  launcher's grid (Flatpak export dirs are in `XDG_DATA_DIRS`).
- **Windows binary.** `compat.run_exe` (default prefix) or
  `compat.run_proton` (per-app data dir). Lifecycle tracked;
  `compat.list_running` / `compat.terminate` available.

## Phase 11 candidates

Pick one or two of these; the loop above is solid, so further work
is about depth (compositor, anti-spoof) or polish (UX, smoke
tests, docs).

1. **Lilith popup with full streaming view.** The bar input
   placeholder is fine for the one-line case; a real conversation
   surface (history of replies, chain-step badges, expandable
   tool results) would let the user see what's happening when a
   chain runs longer.
2. **openWakeWord ONNX hotword.** Replace sliding-window Whisper
   with a dedicated wake-word model. Latency drops <300 ms; CPU
   ~5 %. Needs `ort` crate + an "oi lilith" model (training or
   substitution).
3. **x-vector / d-vector voiceprint.** Anti-spoofing replacement
   for the MFCC+DTW matcher. ONNX-based, ~10 MB model.
4. **Smithay compositor — real surfaces.** Phase 4 left the
   scaffold compiling; Phase 11 candidate: input handling, output
   management, xdg-shell surfaces, single-monitor parity with
   labwc. Multi-week.
5. **Voice at the greeter (pre-login).** Cached voiceprints at
   `/var/lib/jarvis-auth/<user>/` or a system-bus parallel
   interface on the voice daemon so PAM-from-greetd can reach
   biometric verification before the session bus exists.
6. **Lilith test harness.** Mock Ollama; cover the chain loop,
   the rule parser, the audit emission. Confidence > 0; today
   it's "compiles + ad-hoc tested on a VM".
7. **Documentation pass beyond docs.** Pull the action catalogue
   into a generated reference; ADR cross-links; SDK author guide.

## Success criterion for whatever is picked

Same as previous phases: the chosen item ships in a buildable ISO
or workspace, has at least one ADR or module.md update where
appropriate, and the end-to-end loop it touches keeps working
afterwards.
