# ADR 0013: Windows Compatibility Layer — Daemon Owning Wine

## Status
Accepted

## Context

`jarvis-core-context.md` lists "Compatibility-oriented (Windows apps
should feel native via Wine/Proton)" among the project's defining
properties. The Action Bus has reserved `app.install` / `app.uninstall`
slots since day one for the compatibility layer, sitting as stubs.

Bazzite's Wine/Proton setup is the reference. The pragmatic Phase 3
question is: where does the "I have a Windows .exe, run it" logic
live, and how does Lilith / the rest of the desktop talk to it?

## Decision

Build a new system daemon **`system/compat/`** that exposes
`com.jarvis.Compat` over the session bus. V1 implements a single
method, `RunExe(path, args)`, which spawns the binary under
`wine` with an isolated WINEPREFIX. The Action Bus exposes the
matching `compat.run_exe` action; Lilith gets it as a tool for free.

Wine prefix management for V1 is one shared default prefix at
`~/.jarvis/wine/default/`. Per-app prefixes, prefix versioning,
DXVK / VKD3D opt-in, and Proton support are V2 work.

## Reasons

- **Daemon, not a library inside Lilith or the shell.** Same reason
  Voice / Notifications are daemons: long-lived Windows processes are
  managed by something whose only job is managing them. A crash in
  Wine cannot kill chat or kill the shell.
- **Single shared prefix in V1.** Most Windows utilities run cleanly
  in a shared prefix; the prefix-per-app model is for games and
  heavyweight productivity apps. Building the full Bottles-like
  catalog now would balloon scope. V1 proves the wiring; V2 splits
  prefixes when an app actually needs it.
- **Wine, not Proton, in V1.** Proton requires the Steam runtime as
  a dependency and is targeted at games. Stock wine covers
  productivity apps with one dnf package. Phase 4+ adds Proton when
  the gaming story matters.
- **Dangerous permission scope.** Running an arbitrary Windows
  binary is a destructive-adjacent action (it can install services,
  modify the user's Wine prefix, talk to the network). The
  `compat.run` scope joins `terminal.execute` and friends in the
  Permission System's RequireGrant bucket. The user approves the
  first time Lilith asks; the approval persists.

## Consequences

- New crate `system/compat/` in the workspace. Daemon binary
  `jarvis-compat`, systemd user unit `Wants=` on `jarvis-session.target`
  (advisory — Lilith / shell / SDK apps still come up if Wine is
  broken; `compat.run_exe` just returns UNAVAILABLE).
- New `compat.*` action namespace on the Action Bus: V1 has just
  `compat.run_exe`. V2 will add `compat.list_apps`, `compat.install_app`,
  `compat.create_prefix`.
- ISO grows by Wine's footprint (~200 MB compressed). Worth it: the
  bible's promise of Windows-app compatibility lands here.
- `app.install` / `app.uninstall` remain Action Bus stubs because the
  Windows-app installer is V2. V1 is just "run an .exe the user
  already has".

## V1 vs V2

| Item | V1 (this) | V2 |
|---|---|---|
| Engines | wine | + Proton via the Steam runtime |
| Prefix model | shared `~/.jarvis/wine/default/` | per-app with `compat.create_prefix` |
| App catalog | none — caller passes a path | metadata + `.lnk` parsing + start-menu integration |
| DXVK / VKD3D | wine's bundled fallback | toggle per-prefix |
| Lifecycle | fire-and-forget spawn | tracked PIDs, `compat.list_running`, `compat.terminate` |
| Action Bus surface | `compat.run_exe` | + install / uninstall / list |

## Alternatives Considered

- **Wrap Bottles directly.** Rejected: bottles is a heavy Python +
  GTK stack, plus we'd own none of the integration surface. Better
  to keep our own daemon and *optionally* adopt bottles' app catalog
  format later.
- **Make Lilith spawn wine herself.** Rejected: same coupling
  problem as voice / notifications. Process management belongs in a
  daemon that exists to do that.
- **Lump Windows-app installer into V1.** Rejected: scope. Run-only
  ships now; install lands when we have a story for sourcing the
  installers (Lutris-style script catalog? user-provided MSI?).
