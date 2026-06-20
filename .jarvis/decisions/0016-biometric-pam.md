# ADR 0016: Biometric Auth Through PAM, Not the Greeter

## Status
Accepted

## Context

The greeter, the lock screen, and `sudo` all need an auth verdict.
Today the greeter uses greetd's PAM-backed socket protocol, the lock
screen shells out to `pamtester`, and `sudo` is plain Linux PAM.
Adding voice / face unlock is *three different integrations* unless
we pick a single point of authority.

We need to answer: where does the biometric check live?

## Options Considered

1. **In the greeter / lock window directly.** Each surface does its
   own voice capture, calls `com.jarvis.Voice.VerifyVoiceprint`, and
   decides locally. Maps poorly to `sudo`, which would never see
   biometric. Three implementations, all subtly different.
2. **In Lilith.** Use the assistant as the auth oracle. Sounds neat
   until you remember Lilith is a chat daemon, not a security
   boundary. Bad fit.
3. **In a custom PAM module.** Every Linux auth surface — greeter,
   lock, sudo, login, gdm — goes through PAM. One module wires
   biometric into all of them.

## Decision

Custom PAM module: `pam_jarvis.so`, lives at `system/pam-jarvis/`.

- One implementation. Adding biometric to a new service is one
  line in its `/etc/pam.d/<service>` file, never a code change.
- The module delegates the actual matching to `com.jarvis.Voice`
  over DBus (V2). The voice daemon already owns the mic; it owns
  voiceprint enrollment + storage too.
- Service configs decide policy (`sufficient` vs. `required`); the
  module decides the mechanic.

## V1 Scope

V1 is a **scaffold that returns `PAM_IGNORE`**. The module is built,
installed at `/usr/lib64/security/pam_jarvis.so`, and tested. It is
deliberately *not* wired into any live PAM config yet — every
service in the ISO continues to use the password path it has today.

This is the safe-by-default posture: the module exists, can be
audited and unit-tested, and V2 fills in the biometric body without
needing a parallel feature flag or a per-service `if biometric_ready`
fork.

## V2 Plan

```
auth   sufficient   pam_jarvis.so   voiceprint
auth   required     pam_unix.so
```

`pam_jarvis.so` reads the `voiceprint` / `faceprint` argv token,
pulls the user via `pam_get_user`, opens a session-bus connection,
calls the matching DBus method on `com.jarvis.Voice` (or its face-id
peer), and returns `PAM_SUCCESS` / `PAM_AUTH_ERR` / `PAM_USER_UNKNOWN`
based on the JSON response.

Concerns to address before V2 ships:

- **Session-bus access from PAM context.** PAM modules run inside
  the calling service's process (greetd as root, sudo as root, lock
  daemon as the user). The session bus isn't trivially reachable
  from a root context. Likely answer: have the PAM module call the
  *system* bus, with the voice daemon publishing a parallel
  `com.jarvis.AuthBiometric` interface there for this use case.
- **Microphone access.** Same root context: how does the voice
  daemon (per-user, session-bus) get tapped? Probably the system-bus
  interface forwards to whichever user session is active per
  logind.
- **Timeout.** PAM has no built-in timeout; biometric capture is
  inherently 1–3 s. The module imposes a 3 s wall.

## Consequences

**Good:**
- Single biometric implementation across every auth surface.
- Standard Linux convention — security folks know to look at PAM.
- Service config is the policy knob; no Jarvis-specific feature flag.

**Bad:**
- PAM modules running biometric talk to a per-session daemon from a
  root context. The system-bus parallel interface adds surface area.
- A misbehaving PAM module can lock the user out of every auth
  surface at once. V1's `PAM_IGNORE` posture exists specifically so
  this risk doesn't apply until V2 has been audited.

## Alternatives Considered

- **fprintd-style separate daemons per biometric.** Standard but
  duplicates work; one of the things LilithOS is trying to avoid
  ("not GNOME + Ollama"). Single owner ⊃ multiple owners.
- **No biometric.** Considered for the "design language is
  cyberpunk-AI" reality of LilithOS — Lilith Mode in the greeter
  needs voice/face to feel honest, not just decorative. Rejected.
