# Current Goals

> Update this file as goals evolve. This is the active work context.

## Current Phase

**Phase 3 — Real OS Surface**

Phase 2 closed the user-facing system: voice, settings, OS upgrades, an
expanded action namespace. Phase 3 fills in what makes Jarvis OS feel
like an *operating system* rather than a shell over Fedora — login,
lock, notifications with real interaction, Windows-app compatibility,
and a third-party SDK so the action layer isn't a closed catalog.

## Decisions Locked

- **Base OS:** Fedora Atomic, OCI image model (ADR 0001, 0005)
- **UI:** Qt6/QML + Wayland (ADR 0002, 0003, 0006)
- **Core IPC:** Jarvis Action Bus via DBus (ADR 0004)
- **Compositor (now):** labwc as a placeholder until our Smithay
  compositor is ready. Decision documented in ADR 0006.
- **First-boot UX:** dedicated updater daemon + splash, not silent
  in-Lilith bootstrap (ADR 0007).
- **App install:** Flatpak/Flathub, `--user` scope (ADR 0009 — Phase 3).
- **Notifications:** Jarvis owns `org.freedesktop.Notifications`
  rather than depending on mako/dunst (ADR 0010).
- **SDK contract:** action manifest discovered at
  `/usr/share/jarvis/apps/` and `~/.local/share/jarvis/apps/`
  (ADR 0011).
- **Greeter:** custom Qt greetd UI instead of an off-the-shelf
  greeter; three modes (Standard / Lilith / Focus) (ADR 0012).
- **Compat:** Wine via per-app prefixes under `~/.jarvis/wine/`;
  Proton deferred to Phase 4 (ADR 0013).
- **Lock:** wlr-layer-shell Overlay + `pamtester` subprocess; full
  `ext-session-lock-v1` deferred (ADR 0014).

## Phase 2 Outcomes (closed)

- ✅ Voice daemon (`com.jarvis.Voice`) — Whisper.cpp STT + Piper TTS,
  cpal actor for the `!Send` audio stream.
- ✅ Action Bus expansion to 28 actions: `browser.open`,
  `clipboard.*`, `screenshot.capture`, `audio.*`, `window.*` stubs,
  `system.notify`, `updater.*`.
- ✅ Updater Phase 2 — `bootc upgrade` driver + `updater.*` actions.
- ✅ Launcher focus restoration fix.
- ✅ Settings daemon (`com.jarvis.Settings`) + shell SettingsPanel,
  SQLite-backed.

## Phase 3 Active Goals (ordered)

1. **Wine compat (V1 + V2).** ✅ Shipped. `com.jarvis.Compat` with
   shared `default` prefix; V2 adds per-app prefixes,
   `CreatePrefix`, `RunExeIn`, `ListPrefixes`. Action Bus entries
   `compat.run_exe`, `compat.run_exe_in`, `compat.create_prefix`,
   `compat.list_prefixes`. Lilith tools wired.
2. **Flatpak app install / uninstall.** ✅ Shipped. `app.install` and
   `app.uninstall` now back onto `flatpak --user`; Flathub remote in
   the ISO. Lilith can install GIMP.
3. **Jarvis SDK + example app.** ✅ Shipped. Manifest schema in
   `sdk/jarvis-sdk-types`, helper crate in `sdk/jarvis-sdk-rust`,
   `examples/jarvis-app-hello` registers one action discoverable
   through the Action Bus at startup.
4. **Custom greeter.** ✅ Shipped V1.5. Three-mode SwipeView
   (Standard / Lilith / Focus), wallpaper + icon assets baked in,
   last-mode persistence, error toast. Anime avatar, voice/face
   PAM, audio-reactive waveform deferred to Phase 4.
5. **Notifications V2.** ✅ Shipped. FreeDesktop `actions[]` honoured
   end-to-end, drawer in the bar with recent history, urgency
   coloring.
6. **Lock screen V1.** ✅ Shipped. `com.jarvis.Lock` daemon +
   `jarvis-lock-window` Qt overlay, `Super+L` keybind via labwc,
   `pamtester` for auth. `lock-ctl` CLI mirror. `ext-session-lock-v1`
   and biometrics deferred.

## Phase 3 Polish (closed in this round)

- ✅ Launcher grid lists Flatpak apps (XDG_DATA_DIRS in
  `jarvis-session-launch` + `DesktopAppsModel.rescan()` on open).
- ✅ Notifications drawer: per-row dismiss + clear-all, daemon emits
  `HistoryChanged` so the shell refreshes without polling.
- ✅ SDK example surfaced in the launcher via
  `/usr/share/applications/jarvis-sdk-hello.desktop`.
- ✅ Idle auto-lock: swayidle in the labwc autostart triggers
  `jarvis-lock-ctl lock` after 5 min.

## Phase 3 Remaining

- ⏳ End-to-end VM smoke test of the full Phase 3 surface (greeter →
  shell → notifications drawer → compat run → lock → unlock). Blocks
  on the next ISO build.

## Phase 4 Backlog (not yet active)

- Smithay-based Jarvis Compositor (replaces labwc).
- Proton + DXVK toggle per Wine prefix; Steam-runtime container.
- Glassmorphism polish pass (blur shaders, surface depth) once the
  compositor is ours.
- Hotword voice activation (`oi lilith`) running off Whisper streams.
- Idle auto-lock (logind input-idle → `com.jarvis.Lock.Lock`).
- Biometric / Face ID / Voice ID PAM modules (custom `pam_*.so`).
- Anime avatar pipeline for the Lilith greeter mode.
- `ext-session-lock-v1` once labwc (or our compositor) supports it.

## Success Criteria for Phase 3

- A user can boot a clean VM, reach the custom greeter (any of three
  modes), log in, install a Flatpak via Lilith, run a `.exe` via
  Lilith, receive a notification with action buttons, click one, and
  press `Super+L` to lock. All without a terminal.
- A third party can drop a manifest under
  `~/.local/share/jarvis/apps/<id>/manifest.json`, restart the
  Action Bus, and have their action callable from Lilith — without
  patching Jarvis OS itself.
