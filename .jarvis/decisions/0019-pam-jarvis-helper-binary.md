# ADR 0019: pam-jarvis V2 — Helper Binary, Session-Bus Best-Effort

## Status
Accepted

## Context

ADR 0016 chose PAM as the place biometric auth lives, shipped V1 as
a `PAM_IGNORE` scaffold, and flagged the open question: how does a
PAM module reach `com.jarvis.Voice` when:

- The PAM module is `dlopen`'d into the calling service's process —
  could be `sudo`, `login`, greetd's helper, screen lock daemon.
- Some of those processes run as root, others as the user.
- The session bus a per-user daemon publishes on lives at
  `/run/user/<uid>/bus` and is **only writable by that uid**.
- `pam_jarvis.so` should not pull a 5 MB tokio + zbus stack into
  every `sudo` invocation's address space.

## Decision

**Helper binary at `/usr/libexec/jarvis-pam-helper`.** The PAM module
stays minimal: it reads `PAM_USER` via `pam_get_user`, `fork`/`exec`s
the helper, and reads its exit code.

```
pam_jarvis.so
  └─ pam_get_user → "alice"
  └─ fork + execv("/usr/libexec/jarvis-pam-helper", "verify", "alice")
  └─ waitpid → exit code
       0 → PAM_SUCCESS
       1 → PAM_AUTH_ERR
       2 → PAM_IGNORE (defer to next module)

jarvis-pam-helper (separate binary)
  └─ tokio runtime, zbus
  └─ getpwnam_r("alice") → uid
  └─ check /run/user/<uid>/bus exists  → if not, exit 2
  └─ ConnectionBuilder::address("unix:path=/run/user/<uid>/bus")
  └─ Proxy::call("VerifyVoiceprint", "alice")
  └─ map JSON → exit 0/1/2
```

### Why a helper, not a library

- **Address-space hygiene.** `sudo` is invoked ~hundreds of times
  per session. A 5 MB shared library that drags in tokio + zbus is
  the wrong default for a process that exists for 50 ms.
- **Failure isolation.** A buggy DBus call can hang/crash the
  helper; the PAM module just reads `waitpid` and returns
  `PAM_IGNORE`. Same crash inside `sudo`'s address space would
  take down `sudo`.
- **Easier ABI.** Exit codes are a stable C contract. Embedded
  DBus call semantics are not — a future zbus version can change
  shape without us touching `pam_jarvis.so`.

### Why session bus, not system bus

The voice daemon is per-user. Voiceprints are stored under
`~/.jarvis/voiceprints.db` (per-user). Publishing a parallel
system-bus interface for biometric verification would let any
process on the system trigger a verification on someone else's
voiceprint — security regression. The user's session bus is the
right scope.

The trade-off: pre-login PAM stacks (the greeter calling PAM before
the user has a session) **can't reach** their target user's session
bus. The helper detects that case and exits 2 (`PAM_IGNORE`); the
PAM stack falls through to the password module. This is the failure
mode we want: voiceprint is opt-in *after* login, password is the
universal floor.

### Failure-open policy

| Helper outcome | PAM return | Why |
|---|---|---|
| exit 0 (match) | `PAM_SUCCESS` | The biometric verdict. |
| exit 1 (mismatch) | `PAM_AUTH_ERR` | The biometric verdict. |
| exit 2 (unavailable) | `PAM_IGNORE` | Defer; password still works. |
| `fork` fails | `PAM_IGNORE` | OS-level pressure, not auth signal. |
| `waitpid` fails | `PAM_IGNORE` | Same. |
| Helper SIGKILL'd | `PAM_IGNORE` | Same. |
| 3 s timeout | `PAM_IGNORE` (via helper) | Daemon hung — don't lock the user out. |

Never fail-closed on infrastructure problems. The PAM stack has a
password module precisely so the user can always get in; biometric
is a faster path on top, not a barrier.

## V2 Wiring Surface

```
# /etc/pam.d/jarvis-lock (Phase 7 — not in V2 ISO yet)
auth   sufficient   pam_jarvis.so
auth   required     pam_unix.so
```

The V2 ISO ships `pam_jarvis.so` + `jarvis-pam-helper` but does NOT
edit any `/etc/pam.d/<service>` file. Operators (or a Phase 7
follow-up) opt in service-by-service. This preserves the safe-by-
default posture: the module exists, the helper exists, but until a
service is explicitly configured to consult them, nothing changes
about that service's auth flow.

## Consequences

**Good:**
- Single place to call from any PAM service.
- Failure-open by default; password module is the floor.
- The voice daemon's existing per-user session-bus design is
  preserved — no parallel security-sensitive system-bus interface.
- ABI between PAM module and helper is exit codes — stable.

**Bad:**
- Two binaries to ship (the .so and the helper). `/usr/libexec/`
  is the right location and standard convention.
- Pre-login PAM stacks (greeter) hit `PAM_IGNORE` every time, which
  is correct but means voiceprint at the *login screen itself* still
  needs separate plumbing (V3 — probably an LD-preloaded session
  bus stub or a tiny enrollment-time face/voice cache).
- A `fork`+`exec` per auth attempt is 5—10 ms. Acceptable for `sudo`
  and screen unlock; would be too slow for very high-frequency auth.

## Alternatives Considered

- **System-bus interface on the voice daemon.** Easier from PAM
  context (no per-user runtime dir lookup), but means any process
  can attempt verification against any user's voiceprint. Rejected
  on security grounds.
- **Bundle zbus directly into pam_jarvis.so.** Bloats the address
  space of every PAM consumer. The performance argument doesn't
  flip until auth becomes very frequent (which is not the desktop
  case).
- **Defer V2 entirely.** Leaves the V1 `PAM_IGNORE` scaffold as
  the only contract for everyone else (settings UI, Lilith tool,
  V2 greeter wiring). Bad scaling trade.
