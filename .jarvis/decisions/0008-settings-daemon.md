# ADR 0008: Dedicated Settings Daemon

## Status
Accepted

## Context

`system.set_setting` and `system.get_setting` were stubbed in the
Action Bus from day one. They cover anything resembling a system
preference — Lilith's preferred reply language, accent colour overrides,
hotkey rebinds, autostart toggles, default-paths, future per-module
configuration. The bus needs to point them at something concrete.

Three shapes were on the table:

1. **Read/write XDG files directly inside the handler** (`~/.config/jarvis/*.toml`).
2. **Reuse Lilith's persistent fact store** (`~/.jarvis/lilith/facts.db`).
3. **A separate `jarvis-settings` daemon** with its own SQLite database
   and DBus interface.

## Decision

Build option 3: a dedicated daemon at `system/settings/` exposing
`com.jarvis.Settings` over the session bus.

## Reasons

- **Single source of truth across processes.** The shell, Lilith, the
  Action Bus, and future modules will all read/write the same settings.
  A daemon mediates concurrent writes and notifies subscribers via DBus
  signals — file-based config or per-process SQLite handles can't do
  that without re-inventing the daemon.
- **Schema discipline.** A daemon owns the storage format. Files lying
  around in `~/.config/` invite ad-hoc parsers and drift.
- **Permission integration is free.** The Action Bus already gates
  `settings.modify` (dangerous) and `settings.read` (safe); the daemon
  just trusts its caller, same shape as Permission and Updater.
- **Lilith's fact store is a separate concern.** Facts are
  user-narrative memory ("favorite editor is vscode"). Settings are
  operational state ("LILITH_MODEL=qwen3:4b"). Conflating them muddies
  both surfaces — different lifecycle, different read patterns,
  different change-notification needs.
- **It's tiny.** ~200 lines of Rust + SQLite + zbus. Same ergonomic
  footprint as `system/permission/`. No over-engineering risk.

## Consequences

- New crate `system/settings/` in the workspace and a new binary
  shipped in the OCI image (~3 MB stripped).
- New systemd user unit `jarvis-settings.service`, wired as
  `Requires=` from `jarvis-session.target` (settings are needed by
  everything; degrading without them is not interesting).
- Action Bus's `system.set_setting` / `system.get_setting` handlers
  become DBus clients of `com.jarvis.Settings`. They keep their
  permission scope (`settings.modify` / `settings.read`) — the daemon
  trusts whatever the bus approves.
- Lilith already exposes both as tools. No Lilith-side change
  required; the existing tool calls start working the day the daemon
  ships.

## Schema

Values are serialized JSON in a single `TEXT` column. The DBus
interface returns the same JSON string so callers parse client-side.
This is the smallest interface that works for strings, booleans,
numbers, and structured values without a tagged-union DBus type.

```
CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL                -- ISO-8601, audit-only
)
```

## Alternatives Considered

- **`~/.config/jarvis/*.toml` files.** Rejected: no change
  notifications, no concurrency guarantees, every consumer re-parses,
  schema drift inevitable. Worth it for OS-wide user-editable config
  (Phase 3 — declarative), not for runtime preferences.
- **Embedding into Lilith.** Rejected: mixes "what the user wants to
  remember" with "how the OS is configured" inside one daemon, and
  ties settings availability to Lilith being up. Lilith can crash
  cleanly; settings must outlive that crash.
- **Per-module config.** Rejected: each daemon parsing its own config
  file is exactly the pattern LilithOS is meant to replace. Action
  Bus + Settings + Permission together are the unified surface.
