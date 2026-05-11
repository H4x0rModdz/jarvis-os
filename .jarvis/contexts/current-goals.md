# Current Goals

> Update this file as goals evolve. This is the active work context.

## Current Phase

**Phase 2 — User-Facing System**

Phase 1 closed the foundation. Phase 2 turns it into something a person
can actually log into and use: real first-boot UX, real voice surface,
real AI agency over the desktop.

## Decisions Locked

- **Base OS:** Fedora Atomic, OCI image model (ADR 0001, 0005)
- **UI:** Qt6/QML + Wayland (ADR 0002, 0003, 0006)
- **Core IPC:** Jarvis Action Bus via DBus (ADR 0004)
- **Compositor (now):** labwc as a placeholder until our Smithay
  compositor is ready. Decision documented in ADR 0006.
- **First-boot UX:** dedicated updater daemon + splash, not silent
  in-Lilith bootstrap (ADR 0007).

## Phase 1 Outcomes (closed)

- ✅ Action Bus daemon (21 actions, JSON-schema dispatch, permission
  consult, audit log, unit tests).
- ✅ Permission System daemon (in-memory grants + approval DBus signal,
  30 s timeout, full unit + e2e coverage).
- ✅ Lilith daemon (Ollama integration, tool calling, intent routing,
  SQLite memory).
- ✅ Qt shell with layer-shell bottom bar, launcher, approval dialog,
  Theme singleton design tokens.
- ✅ Bootable Fedora bootc ISO buildable both locally and in CI.
- ✅ Updater daemon + splash (Phase 1 scope: Ollama model).
- ⏸ Smithay compositor — deferred to Phase 3, labwc serves until then.

## Phase 2 Active Goals (ordered)

1. **Validate the full first-boot demo on a clean VM.** ISO boots → bar
   renders → updater splash appears → model downloads → splash fades →
   user types a question into the bar → Lilith answers.
2. **Voice pipeline.** Whisper-small STT and Piper TTS, both local.
   Push-to-talk first, always-listening hotword later. Without voice,
   the always-present bar is overkill — voice is what justifies its
   shape.
3. **Action Bus namespace expansion.** Today's 21 actions are mostly
   plumbing. Add the user-visible primitives Lilith needs to actually
   *do* things: `browser.open`, `clipboard.set`, `screenshot.capture`,
   `audio.volume`, `window.focus`, `window.tile`, `app.launch`.
4. **Launcher focus restoration.** Deferred bug from Phase 1 — when the
   launcher closes, focus needs to return to the bar's text input.
   Small fix, big UX win.
5. **Updater Phase 2.** Bootc OS upgrade check + `updater.*` actions on
   the Action Bus so Lilith answers "are there updates?" naturally.

## Phase 3 Backlog (not yet active)

- Smithay-based Jarvis Compositor (replaces labwc).
- Wine/Proton integration via bottles or a custom wrapper.
- Jarvis SDK — third-party apps register actions with the Action Bus.
- Custom greeter replacing greetd autologin.
- Glassmorphism polish pass (blur shaders, surface depth) once the
  compositor is ours.

## Success Criteria for Phase 2

- The first-boot demo above runs end-to-end on a clean VirtualBox VM
  with no terminal interaction.
- A user can hold the bar's mic key, speak, and have the command
  dispatched through the Action Bus.
- Lilith can complete an end-to-end task that spans 3+ actions across
  different namespaces (e.g. "tira um screenshot e abre no editor").
