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

## Incident — 2026-11-23: the ISO wiped every disk in the machine

**What happened.** Installing from the ISO on a real PC formatted all connected
disks, not just the target.

**Cause.** `iso/build.toml` carried only a `[[customizations.user]]` block.
With no kickstart of ours, bootc-image-builder generates one for a fully
unattended install; the build manifest of the shipped ISO shows
`"clearpart": {"all": true}` with nothing limiting which disks it applies to,
followed by automatic partitioning and a reboot. This ADR's premise — "Anaconda
already does partitioning, LUKS and user creation reliably", i.e. the user goes
through the normal installer — was never true for our ISO: the installer asked
nothing. In a VM with one empty disk that is indistinguishable from a
convenient install.

**Fix.** `iso/build.toml` now supplies its own kickstart with **no storage
commands**, which switches the builder to "deploy the image, leave the rest to
the kickstart". Anaconda stops at *Installation Destination*; the user picks
the disk and confirms what is erased. Language, keyboard and time zone are
asked too (they were silently en_US / us / UTC). The install ends on the
"complete" screen instead of rebooting by itself. Verified by generating the
manifest with the new config: no `clearpart`, `zerombr`, `autopart`,
`ignoredisk` or `reboot`.

**Guard.** `tools/check-installer-kickstart.sh` fails when the config has no
kickstart of ours, when the kickstart contains any disk-selecting command or a
reboot, and when a generated manifest contains one. It runs in `ci.yml` on
every PR, and in `build-iso.yml` / `tools/build-iso.sh` before the ISO is built
and again on the generated manifest before anything is uploaded.

**Not yet verified:** a real install from a rebuilt ISO. Before it touches
hardware again, boot it in a VM with **two** virtual disks and confirm the
installer stops and asks.

**Every ISO built before this change is unsafe** on a machine with more than
one disk and must not be used or left published.
