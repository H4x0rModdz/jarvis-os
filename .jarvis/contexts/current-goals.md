# Current Goals

> Update this file as goals evolve. This is the active work context.

## Current Phase

**Daily-driver hardening arc (post-Phase-10 reality).** The codebase is far
past the "Phase 10" the history table below records — the macOS-style shell
(top bar + dock + Lilith popup), WiFi/Bluetooth/audio/battery/display panels,
first-boot wizard, proactive AI, OTA, and ~52 actions all ship. The active
work is closing the gap from "impressive demo" to "you can live in it":

- **Arc 1 — AI controls windows (ADR 0025, PR #2).** `window.{focus,minimize,
  maximize,close}` now work on labwc via the shell's `com.jarvis.Shell`
  service (wlr-foreign-toplevel); selected by a `target` string. Geometry /
  snap / workspaces stay deferred to the Smithay compositor.
- **Arc 2 — desktop plumbing (ADR 0026, PR #3).** xdg-desktop-portal (+wlr/gtk)
  for screen-share & file dialogs, udisks2 + udiskie for removable media,
  CUPS + Avahi for printing.
- **Arc 3 — reliability + security (ADR 0027, PR #4).** Graceful AI-offline
  degradation (P002), owner-only memory DBs + LUKS-as-the-at-rest-boundary
  (P003); service auto-restart already in place.
- **Arc 4 — this docs refresh** (current-goals / roadmap / active-problems).

Verification note: `ci.yml` builds only Rust. The Qt shell (Arc 1 C++) and
the image (Arc 2) are proven by `build-iso.yml`, which runs on merge to main.

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

## Next candidates (after the daily-driver arcs)

Already shipped since this list was first written: the Lilith popup with
streaming + chain-step view, and the macOS shell layout (ADR 0022/0024).
What's left, roughly in priority order for "usable OS":

0. **Land the open PRs.** Merge #2/#3/#4 to main so `build-iso.yml` compiles
   the Qt shell (proves Arc 1's C++) and builds the image with the new
   plumbing (proves Arc 2). This is the real verification gate.
1. **Smithay compositor — real surfaces (P001, still open — needs an ADR).**
   The prime mover: it unblocks the `window.{move,resize,snap_*}` +
   `workspace.*` actions Arc 1 deferred, plus compositor-controlled blur.
   Input/output management, xdg-shell, single-monitor parity with labwc.
   Multi-week.
2. **Multi-monitor + HiDPI scaling depth.** `DisplayPanel` exists; confirm
   2+ monitor arrangement and fractional scaling actually work on labwc
   (notebook + external monitor is the common case).
3. **Installer: LUKS + user creation.** Ties off ADR 0027's at-rest
   decision — disk encryption is the real privacy boundary; a release build
   also needs to drop the dev `jarvis/jarvis` default.
4. **openWakeWord ONNX hotword** — dedicated wake model, <300 ms latency.
5. **x-vector / d-vector voiceprint** — anti-spoofing over MFCC+DTW.
6. **Voice at the greeter (pre-login)** — biometric verify before the
   session bus exists.
7. **Test-harness + generated docs** — broaden coverage beyond lilith/voice;
   generate the action catalogue reference; SDK author guide.

## Success criterion for whatever is picked

Same as previous phases: the chosen item ships in a buildable ISO
or workspace, has at least one ADR or module.md update where
appropriate, and the end-to-end loop it touches keeps working
afterwards.
