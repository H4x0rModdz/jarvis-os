# 0030 — Light installer rebrand + LUKS as the at-rest boundary

Status: accepted
Date: 2026-06-20

## Context

Two backlog items from the legal/privacy review remained: a "custom installer"
(ADR 0029 Phase B) and "encryption at rest". Looking at both honestly:

- **Bespoke installer** would mean reimplementing partitioning + LUKS +
  bootloader — the most dangerous code in an OS, unverifiable off a VM (it wipes
  disks), and **purely cosmetic**: Phase A (ADR 0029) already delivered the real
  install-time goals (user password via first-boot, consent/opt-in screens), and
  Anaconda already does partitioning, LUKS and user creation reliably. Building a
  partitioner from scratch is exactly the overengineering the project rejects.
- **At-rest encryption** of the voiceprint / AI stores needs a secret to derive
  a key from. We ship **autologin** (ADR 0029: fixed `jarvis` user), so no
  password is entered at boot — app-level encryption (SQLCipher) would be
  "security theatre", which is precisely what **ADR 0027** already concluded.

## Decision

1. **Rebrand the installer (and the whole OS identity) via `os-release`, not a
   bespoke installer.** The final image rewrites `NAME`/`PRETTY_NAME` in
   `/usr/lib/os-release` to `LilithOS`. Anaconda shows "Install
   <PRETTY_NAME>", and os-release is what `bootc status`, the greeter and
   fastfetch read — so this one safe edit rebrands the installer chrome and the
   running system. `ID`/`VERSION_ID` stay Fedora so `$releasever`, dnf and any
   ID-based logic keep working. This **supersedes ADR 0029 Phase B** (the
   from-scratch Qt installer) as the chosen path; Phase B remains a far-future
   option, not planned.

2. **LUKS (full-disk) is the at-rest boundary; app-level crypto is not added.**
   This re-affirms ADR 0027. The real protection against a powered-off stolen
   disk is LUKS, set up interactively in the Anaconda installer (its "Encrypt my
   data" option). We **document and encourage** it (README / install notes)
   rather than baking a passphrase (which can't be done securely). We do **not**
   add SQLCipher to the SQLite stores: under autologin its key would have to live
   on the machine, adding key-management complexity for no real gain.

3. **Genuine per-user app-level encryption stays gated on dropping autologin.**
   If we ever want the voiceprint / AI databases encrypted with a key derived
   from the user's login (so a local attacker without the password can't read
   them), that requires switching from autologin to a real greeter login that
   unlocks a keyring. That's a deliberate UX change, recorded here as the
   condition — not done now.

## Consequences

- The installer and system identify as **LilithOS** (one `sed` on os-release),
  with zero disk-code risk. Needs a quick on-device check (boot the ISO, confirm
  the installer title) — string-level, low risk.
- At-rest protection = **LUKS at install** (user-chosen), consistent with ADR
  0027. The privacy/consent opt-ins (ADR 0029) cover the *collection* side;
  LUKS covers the *at-rest* side. Together that's the LGPD posture.
- No new dependencies, no SQLCipher, no bespoke installer to maintain.
- A deeper installer logo/theme swap (Anaconda branding pixmaps) is possible
  later but needs ISO-build plumbing; deferred.

## Alternatives rejected

- **Bespoke Qt installer via `bootc install to-disk` (ADR 0029 Phase B)** — weeks
  of disk-wiping code, unverifiable off-device, cosmetic over Phase A + Anaconda.
- **App-level SQLCipher under autologin** — theatre (ADR 0027); key has nowhere
  safe to live without a login secret.
- **Baking a LUKS passphrase via kickstart** — a baked passphrase is not a
  secret; interactive LUKS at install is the only honest path.

## Related

- ADR 0027 (at-rest hardening — LUKS over app-level crypto).
- ADR 0029 (custom installer — Phase A shipped; this supersedes Phase B).
- `iso/Containerfile` (os-release rewrite), `iso/build.toml`, `iso/README.md`.
