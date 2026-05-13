# pam-jarvis

## Purpose

A custom PAM auth module — `pam_jarvis.so` — that adds biometric
checks (voiceprint + faceprint) to the system's PAM stack. The
greetd login flow, the lock screen, and `sudo` all share the same
PAM stack on Linux; one module wires biometric into all of them.

## Status

**V2 — wired through to `com.jarvis.Voice`.** The PAM module reads
`PAM_USER` via `pam_get_user`, `fork`/`exec`s
`/usr/libexec/jarvis-pam-helper`, and translates exit codes into
the standard `PAM_SUCCESS` / `PAM_AUTH_ERR` / `PAM_IGNORE` returns.
The helper does the heavy DBus call so the PAM `.so` stays small —
ADR 0019 explains the helper-binary architecture.

No `/etc/pam.d/<service>` file in the V2 ISO references the module
yet. Operators opt in service-by-service; safe-by-default until then.

## Boundaries

- The module **owns** the biometric verdict surface. Any service that
  trusts biometric auth on Jarvis OS goes through `pam_jarvis.so`,
  not through ad-hoc DBus calls. This means a future "lock screen
  unlocks with voice" landing automatically lights up on `sudo` too.
- The module **does not** run the matching itself. V2 calls into
  `com.jarvis.Voice.VerifyVoiceprint(user)` (and a peer interface
  for faceprint); voiceprint enrollment + storage lives in the voice
  daemon, not here.
- The module **does not** replace passwords. V2 returns `PAM_SUCCESS`
  on a biometric match — equivalent to a password — and `PAM_AUTH_ERR`
  on a mismatch, which lets the next module in the stack try. The
  service config decides whether biometric is `sufficient` (skip
  password on match) or `required` (still need password too).

## Interface

PAM modules don't have a DBus surface — libpam dlopens them and
calls into the C ABI symbols `pam_sm_authenticate` and
`pam_sm_setcred`. See `src/lib.rs` for the signatures.

The module is loaded by adding a line to the PAM service file:

```
# /etc/pam.d/jarvis-lock (planned V2)
auth   sufficient   pam_jarvis.so   voiceprint
auth   required     pam_unix.so
```

V2 reads the first argv token (`voiceprint` / `faceprint`) to pick
which biometric to attempt; V1 ignores argv entirely.

## V1 vs V2 vs V3

| Item | V1 | V2 | V3 (current) | V4 |
|---|---|---|---|---|
| `pam_sm_authenticate` | Always `PAM_IGNORE` | exec `jarvis-pam-helper verify <user>` (helper calls `com.jarvis.Voice.VerifyVoiceprint`) | unchanged | + `faceprint` argv branch against a face-id daemon |
| Helper transport | n/a | session bus at `/run/user/<uid>/bus`, 3 s wall | unchanged | + cached enrollment for pre-login services |
| `pam_sm_setcred` | Always `PAM_SUCCESS` | unchanged | unchanged | unchanged |
| Enrollment | none | `EnrollVoiceprint` DBus | + `jarvis-voiceprint-ctl` CLI + Settings UI biometric section | + face enrollment via the camera |
| Service wiring | not wired into any live PAM config | not wired (operator opt-in) | `/etc/pam.d/jarvis-lock` references `pam_jarvis.so sufficient` (ADR 0020); other services untouched | + greeter, sudo (after Phase 8 lock-window voice button) |

## Failure Modes

| Failure | Behavior (V1) | Behavior (V2 plan) |
|---|---|---|
| `com.jarvis.Voice` offline | n/a (V1 doesn't call) | Return `PAM_AUTH_ERR`; service falls back to next module. |
| User not enrolled | n/a | Return `PAM_USER_UNKNOWN`; service falls back to password. |
| Mic busy / can't capture | n/a | Return `PAM_AUTHINFO_UNAVAIL`; service falls back. |
| Daemon hung during call | n/a | DBus call timeout (3 s) → `PAM_AUTH_ERR`. |
