# Jarvis Settings

## Purpose

Stores the system's runtime preferences in one place — Lilith's
preferred model, default reply language, accent colour overrides,
hotkey rebinds, autostart toggles, anything that "the OS should
remember between reboots that isn't a user-narrative fact."

User-narrative memory ("favorite editor is vscode") lives in Lilith's
fact store, **not here**. See [ADR 0008](../../.jarvis/decisions/0008-settings-daemon.md)
for the split rationale.

## Boundaries

- Settings **stores arbitrary JSON values keyed by string.** It does
  not validate per-key schemas — that's a callers' concern.
- Settings **does not gate access.** Callers go through the Action Bus
  (`system.set_setting` / `system.get_setting`), which gates on the
  `settings.modify` / `settings.read` scopes via the Permission System.
  The daemon itself trusts whatever lands on its socket.
- Settings **does not silently provide defaults.** A `Get` on a missing
  key returns `{ "found": false }` and the caller decides the default.
  This keeps the system's "what is the user's actual choice" surface
  honest.

## Interface

```
DBus  com.jarvis.Settings  at  /com/jarvis/Settings

  Get(key: string) -> string  // JSON
       └─ { found: bool, key: string, value?: <any JSON> }

  Set(key: string, value_json: string) -> string  // JSON
       └─ stores `value_json` verbatim (must parse as JSON)
       └─ returns { ok: bool, error?: string }

  Delete(key: string) -> string  // JSON
       └─ { deleted: bool }   // false when the key didn't exist

  List() -> string  // JSON
       └─ { keys: [{ key, updated_at }] }  // values omitted to keep
                                            // the list cheap; callers
                                            // Get() what they need.

  signal Changed(key: string, value_json: string)
       └─ fires on every successful Set or Delete (Delete emits with
          an empty value_json). Subscribers reconcile their cached
          state without polling.
```

`value_json` is a JSON document — `"42"`, `"\"qwen3:4b\""`,
`"{\"theme\": \"dark\"}"`, `"true"`. The daemon validates it parses;
the schema of the content is the caller's responsibility.

## Storage

SQLite at `~/.jarvis/settings.db`. Single table:

```sql
CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

The database is opened with WAL mode so the daemon doesn't block its
own concurrent reads. Writes are synchronous (`PRAGMA synchronous=NORMAL`)
— a setting written and then read back must reflect the new value.

## Failure modes

| Failure | Behavior |
|---|---|
| Daemon unreachable | Action Bus handlers return `UNAVAILABLE`; callers see the bus's error envelope, not a partial result. |
| `value_json` doesn't parse | `Set` returns `{ ok: false, error: "invalid JSON: …" }`. No partial write. |
| SQLite I/O error | `Set` returns `{ ok: false, error: "<sqlite message>" }`. Caller may retry. |
| Database file missing on first run | Daemon creates it (and `~/.jarvis/`) at startup. No bootstrap step required. |
