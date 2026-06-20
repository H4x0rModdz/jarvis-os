# Architecture: Filesystem Philosophy

## Purpose

Define how LilithOS structures its filesystem for clarity, discoverability, and AI compatibility.

## Core Principle

The filesystem should be self-explanatory. A new user or AI agent should be able to navigate it without documentation.

## User Directory Structure

```
~/.jarvis/
  config/           ← user settings (human-readable TOML)
  data/
    lilith/
      memory/       ← session + persistent memory databases
      automations/  ← user-created automation definitions
    apps/           ← app-specific data (sandboxed per app)
  compat/
    prefixes/       ← Wine/Proton prefixes
    cache/          ← DXVK shader caches, runner caches
  logs/             ← system + AI audit logs
  permissions.db    ← permission grants store
  cache/            ← temporary, safe to delete
```

## System Directory Structure

```
/usr/lib/jarvis/
  ai/               ← Lilith daemon, models
  shell/            ← compositor, window manager, taskbar
  system/           ← action bus, permission daemon
  compat/           ← Wine runners, Proton versions
  sdk/              ← developer SDK libraries

/etc/jarvis/
  system.toml       ← system-wide defaults
  allowed-actions.toml ← default action policy

/var/log/jarvis/
  audit.log         ← system-level action audit log
```

## File Format Preferences

| Use Case | Format |
|---|---|
| Configuration | TOML |
| Action schemas | JSON |
| Module metadata | Markdown + YAML frontmatter |
| Databases | SQLite |
| Logs | JSON Lines (one JSON object per line) |
| IPC messages | JSON over Unix socket |

Never XML. Never INI (unless compatibility requires it).

## Naming Conventions

- Directories: `snake_case`
- Config files: `snake_case.toml`
- Log files: `service_name.log` or `service_name_YYYY-MM-DD.log`
- No spaces in any path Jarvis creates

## AI Access Rules

Lilith can read/write within:
- `~/.jarvis/data/lilith/` — unrestricted
- `~/.jarvis/config/` — read only by default, write with settings.modify permission
- User-approved paths — with filesystem.read/write permissions

Lilith cannot access:
- `/etc/`, `/boot/`, `/usr/` — system-off-limits
- Other users' home directories
- App sandbox directories of apps that haven't granted access
