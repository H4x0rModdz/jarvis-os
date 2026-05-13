# ADR 0020: First Shipping Wiring of `pam_jarvis.so` — `jarvis-lock`

## Status
Accepted (amended Phase 8 — see "V2 update" below).

## V2 update (Phase 8)

V1 of this wiring put `pam_jarvis.so sufficient` at the top of
`/etc/pam.d/jarvis-lock` so a voice match would short-circuit the
password path. The downside was real: every typed-password unlock
waited ~2.5 s for the voice attempt to time out before pam_unix ran.

Phase 8 splits the stacks:

- `/etc/pam.d/jarvis-lock` — password-only. `auth include
  system-auth`, no `pam_jarvis.so`. Typed unlocks are instant again.
- `/etc/pam.d/jarvis-lock-voice` — `auth required pam_jarvis.so`.
  Voice-only; no password fallback on this path (the call carries no
  password). Lock daemon's new `VerifyVoice()` method uses this
  service.

The lock daemon routes by entry point:

```
com.jarvis.Lock.Verify(password)      → pamtester jarvis-lock       (password)
com.jarvis.Lock.VerifyVoice()          → pamtester jarvis-lock-voice (voice)
```

The Qt lock window keeps the password field as the front-and-center
default and exposes a "🎙 Falar para desbloquear" pill below it. Voice
is opt-in per unlock attempt; typed-password is one keystroke.

The original V1 text below is preserved for history.

---


## Context

ADR 0019 shipped `pam_jarvis.so` + `jarvis-pam-helper` but left every
`/etc/pam.d/<service>` untouched — safe-by-default until the
biometric stack was end-to-end. Phase 7 finished the loop:

- Voice daemon `EnrollVoiceprint` / `VerifyVoiceprint` (Phase 5)
- MFCC + DTW matcher (Phase 6)
- pam-jarvis V2 helper-binary architecture (Phase 6)
- Settings UI for enrollment (this phase)
- `jarvis-voiceprint-ctl` CLI (this phase)

It's time to wire one real service. The lock screen is the right
first target:

- Per-user. The voice daemon runs in the user's session bus already.
- Failure-open is well understood (password fallback is the floor).
- Lower stakes than `sudo` / `login` — a misfire just makes the user
  type their password.

## Decision

Ship `/etc/pam.d/jarvis-lock` with the voiceprint as `sufficient`:

```
auth        sufficient   pam_jarvis.so
auth        include      system-auth
account     include      system-auth
password    include      system-auth
session     include      system-auth
```

Lock daemon switches `pamtester` from the `login` service to
`jarvis-lock`, so unlock attempts now route through this stack.

### What the user sees

- **Voice match.** `pam_jarvis.so` returns `PAM_SUCCESS`. The
  sufficient rule ends the stack; lock daemon's `Verify(password)`
  returns ok without `pam_unix` being consulted. The user typed
  nothing, mouthed `oi lilith`, the screen unlocks.
- **Voice miss / no enrollment / daemon offline.** Helper exits
  `PAM_AUTH_ERR` (miss) or `PAM_IGNORE` (unavailable). Stack
  continues to `system-auth`; `pam_unix` reads the password from
  stdin (lock daemon piped it in already) and authenticates the
  classical way.

### Trade-off

A typed-password unlock now pays a one-time wait for the voice
attempt to complete or time out. Concretely: the helper has a 3 s
budget, and a non-enrolled or non-speaking user hits the `PAM_IGNORE`
path in about that long before the password check runs. Acceptable
for V1 — the user gets a working unlock either way, just slower than
before by a couple of seconds when they chose to type.

The right long-term fix is **two unlock paths in the lock window**:

- Default focus on the password field. Typing immediately routes
  through a password-only PAM service that has no voice rule, so
  there's zero added latency.
- A separate "Falar para desbloquear" button that calls a new
  `VerifyVoice()` daemon method using the `jarvis-lock` service
  (voice path), letting the user opt into the voice attempt
  explicitly.

That's Phase 8 work. V1 documents the latency honestly here and in
`system/lock/module.md` so nobody is surprised.

## Safety Constraints That Hold

- Password fallback is always the floor. No combination of voice
  miss, daemon crash, or PAM module misbehaviour locks the user
  out, because `pam_unix` is `required` further down the stack.
- The voice matcher (MFCC + DTW; ADR 0018 V2 update) is beatable
  by recording-playback. We don't gate anything higher-stakes than
  the lock screen on it. `sudo`, `login`, GDM all keep the default
  Fedora PAM stack untouched.
- Failure modes are documented uniformly: helper exit 2 → PAM_IGNORE
  → next module. There is no scenario where a misbehaving helper
  short-circuits to PAM_SUCCESS.

## Consequences

**Good:**
- End-to-end voice unlock: enroll in Settings, lock the screen,
  speak — it unlocks. The system feels AI-native at the boundary
  where biometric matters most.
- Single shipping wiring in V1 — easy to audit and revert.
- The architecture (`pam_jarvis.so` + helper + voice daemon) is
  exercised in a real flow, surfacing any latent bugs before
  Phase 8 expands the surface to other services.

**Bad:**
- Typed-password unlocks slower by ~2.5 s until lock-window V2.
- A misbehaving voice daemon could induce that latency on every
  unlock; the only mitigation in V1 is the helper's 3 s timeout.

## Alternatives Considered

- **Ship the PAM config but keep the daemon on `login`.** Adds the
  file without enabling it; demonstrably half the work, no end-to-
  end flow, no real exercise of the wiring. Rejected as theatre.
- **Switch only after lock-window V2 lands the dedicated voice
  button.** Pushes Phase 7's deliverable into Phase 8. The latency
  is real but well-bounded; better to ship the flow and tune.
- **Voice as `required` not `sufficient`.** Locks out users who
  haven't enrolled. Non-starter for default config.
