# Jarvis Lock (system daemon)

## Purpose

Owns `com.jarvis.Lock` on the session bus. Two responsibilities:

1. **Track locked state.** One boolean source of truth, emitted on
   change as `LockStateChanged`. The shell binds to this so the bar's
   lock button (and any future affordance) can reflect the session
   state without polling.
2. **Authenticate password attempts.** The Qt lock window
   (`jarvis-lock-window`) sends `Verify(password)`; the daemon shells
   out to `pamtester` against the system PAM stack and answers
   `{ ok: bool, reason?: string }`.

Spawning the lock window happens here too — `Lock()` launches
`jarvis-lock-window` as a child process so the daemon owns its
lifetime: a crash transitions us back to unlocked instead of
stranding a dead overlay.

See [ADR 0014](../../.jarvis/decisions/0014-lock-screen.md) for the
rationale (why a daemon, why `pamtester`, why not ext-session-lock-v1
in V1).

## Boundaries

- The daemon **owns** the locked flag. Every other component reads it
  through `IsLocked()` or `LockStateChanged`; nobody else writes.
- The daemon **does not** link libpam. It calls `pamtester login
  <user> authenticate` as a subprocess. Same auth path (PAM stack
  unchanged), no bindgen + libclang cascade.
- The daemon **does not** draw. The Qt overlay (`jarvis-lock-window`,
  see [shell/jarvis-lock/module.md](../../shell/jarvis-lock/module.md))
  is a separate binary it spawns and tears down.
- The daemon **does not** approve the call. The Action Bus's
  `system.lock` action gates through the Permission System; the
  daemon trusts whatever DBus client lands on its socket.

## Interface

```
DBus  com.jarvis.Lock  at  /com/jarvis/Lock

  Lock()             -> string   // JSON { ok, already?, reason? }
       └─ Idempotent. Spawns jarvis-lock-window if not already locked.

  Verify(password)   -> string   // JSON { ok, reason? }
       └─ Called by the lock window on submit. Routes through the
          `jarvis-lock` PAM service (password-only). Success
          transitions the daemon to unlocked and kills the lock
          window.

  VerifyVoice()      -> string   // JSON { ok, reason? }
       └─ Called by the lock window's voice pill. Routes through
          `jarvis-lock-voice` (pam_jarvis.so required, no password
          fallback). Daemon captures 2 s, scores, returns verdict.

  IsLocked()         -> bool

  signal LockStateChanged(locked: bool)
```

## Behavior

| Trigger | Action |
|---|---|
| `Lock()` while unlocked | Spawn `/usr/bin/jarvis-lock-window`; mark locked; emit `LockStateChanged(true)`. |
| `Lock()` while locked | No-op; returns `{ ok: true, already: true }`. |
| `Verify(pw)` correct | PAM succeeds; kill lock window (child); mark unlocked; emit `LockStateChanged(false)`. |
| `Verify(pw)` wrong | PAM fails; daemon returns `{ ok: false, reason: "Senha incorreta" }`; lock window stays up. |
| Lock window crashes | Child wait completes with non-zero; daemon clears locked flag and emits `LockStateChanged(false)` — fail-open, matching the wider screen-locker convention (a locker that traps the user when it crashes is worse than no locker). |
| `pamtester` missing | `Verify` returns `{ ok: false }` and logs at WARN. The lock window shows "Senha incorreta" — visible failure beats silent unlock. |

## PAM stacks (Phase 8 split)

Two services, routed by entry point:

| Entry point | Service file | Auth body |
|---|---|---|
| `com.jarvis.Lock.Verify(password)` | `/etc/pam.d/jarvis-lock` | `auth include system-auth` (password only) |
| `com.jarvis.Lock.VerifyVoice()` | `/etc/pam.d/jarvis-lock-voice` | `auth required pam_jarvis.so` (voice only) |

Typed-password unlocks now go through `jarvis-lock` and never touch
the voice helper — the ~2.5 s latency the Phase 7 wiring introduced
is gone. The Qt lock window keeps the password field as the
default focus and exposes a "🎙 Falar para desbloquear" pill that
explicitly calls `VerifyVoice` when the user wants the voice path.

ADR 0020 (amended for Phase 8) records the split rationale.

## Auto-lock on idle

Driven by `swayidle`, supervised by the daemon. `idle_lock_supervisor`
reads `lock.idle_timeout_seconds` from `com.jarvis.Settings`, spawns
swayidle with that timeout, and respawns whenever the setting
changes. swayidle speaks `ext-idle-notify-v1` to the compositor and
runs `jarvis-lock-ctl lock` once input has been idle long enough.
The lock daemon's `Lock()` is idempotent, so racing with `Super+L`
is harmless.

Default timeout: 300 s. Zero disables auto-lock — supervisor kills
swayidle and doesn't respawn. SettingsPanel.qml exposes the slider;
changes take effect immediately.

## V1 Limitations (deferred)

| Item | Why deferred |
|---|---|
| `ext-session-lock-v1` | labwc didn't ship the protocol when V1 landed; layer-shell Overlay + KeyboardInteractivityExclusive covers everyday lock UX with one caveat (VT switch escapes). |
| Biometric / Face ID / Voice ID | Needs custom `pam_*.so` modules. Same blocker as the greeter V2. |

## Failure Modes

| Failure | Behavior |
|---|---|
| `/usr/bin/jarvis-lock-window` missing | `Lock()` returns `{ ok: false, reason: "lock window binary missing at …" }`. Caller decides whether to retry / notify. |
| `pamtester` not on PATH | `Verify` returns `{ ok: false }`. Logged at WARN. |
| Daemon crashes while locked | systemd `Restart=on-failure` brings it back; locked state is in-memory so we come back unlocked. Fail-open same as crash-of-child case. |

## Files

- `system/lock/src/main.rs` — the entire daemon. ~180 lines.
- `iso/assets/systemd/jarvis-lock.service` — user unit; `Type=dbus`,
  `BusName=com.jarvis.Lock`, `WantedBy=jarvis-session.target`.
- `iso/assets/labwc/rc.xml` — `Super+L` keybind dispatches
  `jarvis-lock-ctl lock`.
- `tools/lock-ctl/` — tiny CLI mirror of the DBus surface.
