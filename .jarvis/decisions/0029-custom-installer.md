# 0029 — Custom LilithOS installer (user/password, LUKS, feature consent)

Status: accepted
Date: 2026-06-20

## Context

Today the ISO is produced by `bootc-image-builder`, which ships **Anaconda** as
the installer, and `iso/build.toml` **bakes a default user** `jarvis/jarvis`
into the image. Three problems, all surfaced by the legal-risk review:

1. **Default credentials.** `jarvis/jarvis` + SSH password auth is a security
   liability the moment an image touches real hardware (ADR 0027 flagged it as a
   release blocker). The user must set their own password at install.
2. **No real at-rest encryption.** ADR 0027 decided app-level crypto is theatre
   on an **autologin** box because there's no user secret to derive a key from.
   A real install-time **password unlocks real encryption** (LUKS for the disk;
   later a login-derived key for the biometric/AI stores). The installer is the
   keystone that makes the LGPD encryption work non-theatre.
3. **No consent / opt-in surface.** Biometric voiceprint, hotword, AI memory are
   sensitive (LGPD). The user must **explicitly choose** what to enable, not
   inherit it silently.

The product wants "our own installer instead of Fedora's default." The honest
engineering question is *how much* to build ourselves: a from-scratch
partitioner + LUKS + bootloader path is the **dangerous** part (it writes
disks), and Anaconda already does it reliably.

## Decision

**Phase the installer. Deliver the user-facing goals now via configuration + an
owned first-boot setup; defer the full bespoke installer.**

### Phase A — now (low risk, high value)
1. **Stop baking the user.** Remove `[[customizations.user]] jarvis/jarvis`
   from `iso/build.toml` so Anaconda's interactive **user + password** screen
   runs. Kills the default-credential liability.
2. **Offer LUKS at install.** Surface Anaconda's existing disk-encryption option
   (full-disk LUKS). This is the real at-rest boundary (ADR 0027) and unblocks
   the encryption phase — done by the battle-tested installer, not our code.
3. **Owned consent/opt-in, in our stack.** Extend the **existing**
   `FirstBootWizard.qml` (already runs once, self-gates via QSettings) with
   explicit opt-in screens for the sensitive features — voiceprint enrollment,
   "oi lilith" hotword, AI memory/history — each **off by default**, written to
   `com.jarvis.Settings`. This is the "user chooses what to enable" surface, and
   it's safe (writes settings, never touches disks).

Phase A gives every concrete goal — real password, LUKS, feature consent,
branding via the wizard — without writing a single line of partitioning code.

### Phase B — later (ambitious, deferred)
A fully bespoke **Qt installer** (same stack as the greeter/shell, so "even the
installer is LilithOS") that boots a live environment and drives
**`bootc install to-disk`** + `cryptsetup` directly, replacing Anaconda for full
brand control. This owns partitioning/LUKS, so it is the dangerous, high-effort
piece — built only once Phase A's value is shipped and we can test disk flows
safely. `bootc install to-disk` is the bootc-native primitive that makes this
tractable (we orchestrate it, we don't reinvent ostree deployment).

## Consequences

- The default-credential risk is removed at Phase A (no baked user → install
  prompts for one). `tools/dev-deploy.sh` loses its assumed `jarvis/jarvis`; dev
  images can keep a documented dev user behind a build flag, but the default ISO
  prompts.
- Real password exists → the **encryption phase** (next) can derive a key from
  the login (keyring/PAM) instead of being theatre.
- LGPD consent is satisfied by the opt-in wizard; biometric/hotword/memory ship
  **off** until the user turns them on.
- New behaviour in `FirstBootWizard` is documented in
  `shell/jarvis-shell/module.md`; the consent settings keys are added to
  `system/settings`.
- Phase B is a future ADR-tracked effort; this ADR commits only to Phase A now.

## Alternatives rejected

- **Full bespoke installer now.** Reimplementing partitioning + LUKS +
  bootloader is the riskiest code in any OS and we can't safely test it beyond a
  VM yet. The user's goals don't require it — Phase A delivers them. Kept as the
  Phase B north star.
- **Keep baking `jarvis/jarvis`.** A known release blocker (ADR 0027); shipping
  it as the default is a security-negligence liability if it grows.
- **Anaconda addon (Python) for the consent screens.** Heavier and off-stack;
  our Qt FirstBootWizard already exists and is the natural home.

## Related

- ADR 0005 (Fedora Atomic / bootc base), ADR 0007 (first-boot updater).
- ADR 0027 (at-rest hardening — why the password matters for encryption).
- `iso/build.toml`, `shell/jarvis-shell/qml/FirstBootWizard.qml`,
  `system/settings/`.
