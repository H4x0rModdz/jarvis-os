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
       └─ Called by the lock window on submit. Success transitions
          the daemon to unlocked and kills the lock window.

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

## Auto-lock on idle

Driven by `swayidle`, spawned from labwc's autostart with a 5-minute
hardcoded timeout. swayidle speaks `ext-idle-notify-v1` to the
compositor and runs `jarvis-lock-ctl lock` once the input has been
idle long enough. The lock daemon's `Lock()` is idempotent, so a
race against `Super+L` is harmless.

V1 of auto-lock is intentionally non-configurable. V2 reads the
timeout from `com.jarvis.Settings` and respawns swayidle when the
setting changes; until then, editing the labwc autostart is the
only knob.

## V1 Limitations (deferred)

| Item | Why deferred |
|---|---|
| `ext-session-lock-v1` | labwc didn't ship the protocol when V1 landed; layer-shell Overlay + KeyboardInteractivityExclusive covers everyday lock UX with one caveat (VT switch escapes). |
| Biometric / Face ID / Voice ID | Needs custom `pam_*.so` modules. Same blocker as the greeter V2. |
| Configurable idle timeout | Hardcoded 5 min in labwc/autostart; future V2 will pull the value from the Settings daemon. |

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
