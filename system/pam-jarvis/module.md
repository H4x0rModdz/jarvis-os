# pam-jarvis

## Purpose

A custom PAM auth module — `pam_jarvis.so` — that adds biometric
checks (voiceprint + faceprint) to the system's PAM stack. The
greetd login flow, the lock screen, and `sudo` all share the same
PAM stack on Linux; one module wires biometric into all of them.

## Status

**V1 — scaffold only.** The module builds, installs, and is safe to
include in a service's auth stack (it returns `PAM_IGNORE` for every
call, which means "defer to the next module"). The wiring exists; the
biometric checks land in V2.

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

| Item | V1 (current) | V2 | V3 |
|---|---|---|---|
| `pam_sm_authenticate` | Always `PAM_IGNORE` | `voiceprint` arg → DBus call to `com.jarvis.Voice.VerifyVoiceprint(user)`; `faceprint` arg → same against a face-id daemon | + multimodal fusion (voice + face combined score) |
| `pam_sm_setcred` | Always `PAM_SUCCESS` | unchanged | unchanged |
| Enrollment | none | `jarvis-voiceprint-ctl enroll` CLI + a Settings panel section | + face enrollment via the laptop camera |
| Service wiring | not wired into any live PAM config | `jarvis-lock` + `jarvis-greeter` opt in via `sufficient` | + `sudo`, `login`, `gdm` parity |

## Failure Modes

| Failure | Behavior (V1) | Behavior (V2 plan) |
|---|---|---|
| `com.jarvis.Voice` offline | n/a (V1 doesn't call) | Return `PAM_AUTH_ERR`; service falls back to next module. |
| User not enrolled | n/a | Return `PAM_USER_UNKNOWN`; service falls back to password. |
| Mic busy / can't capture | n/a | Return `PAM_AUTHINFO_UNAVAIL`; service falls back. |
| Daemon hung during call | n/a | DBus call timeout (3 s) → `PAM_AUTH_ERR`. |
