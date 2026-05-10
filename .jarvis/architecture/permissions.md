# Architecture: Permission System

## Purpose

Enforces all access controls for AI actions, application capabilities, and sensitive system operations.

## Permission Scope Format

```
<domain>.<operation>[.<target>]

Examples:
  filesystem.read./home/user/documents
  filesystem.write./downloads
  filesystem.delete.*
  app.install
  app.launch.*
  network.request.external
  terminal.execute
  microphone.listen
  window.control
  settings.modify.appearance
```

## Grant Lifecycle

```
Request → User Approval Dialog → Granted (session | persistent)
                               → Denied → Logged
```

Persistent grants are stored in `~/.jarvis/permissions.db`.
Session grants are cleared when the granting session ends.

## Permission Store Schema

```sql
CREATE TABLE grants (
  id          TEXT PRIMARY KEY,
  caller      TEXT NOT NULL,   -- 'lilith', 'app:<id>', 'automation:<id>'
  scope       TEXT NOT NULL,
  granted_at  TEXT NOT NULL,
  expires_at  TEXT,            -- NULL = persistent
  approved_by TEXT NOT NULL    -- 'user_prompt', 'auto_allowed', 'policy'
);
```

## Approval UX Requirements

When a permission is requested:

1. Show the caller (Lilith, specific app, etc.)
2. Show exactly what scope is being requested
3. Show what action triggered the request
4. Show if the action is reversible
5. Offer: Allow Once / Allow Always / Deny

Never auto-approve sensitive scopes (delete, terminal.execute, network.request).

## Dangerous Scopes (Always Require Explicit Approval)

```
filesystem.delete.*
terminal.execute
app.install
app.uninstall
settings.modify.system
network.request.external
camera.access
microphone.listen (first time per app)
```

## Permission Revocation

Users can revoke any grant at any time via:

- Control Center → Permissions panel
- Command palette: "Manage permissions"
- Lilith: "Revoke Lilith's access to downloads"

Revocation takes effect immediately. In-progress actions using the revoked scope are cancelled.

## Audit Integration

Every permission check writes to the audit log regardless of outcome:

```json
{
  "timestamp": "ISO8601",
  "caller": "lilith",
  "scope_requested": "filesystem.delete./downloads/old.zip",
  "outcome": "approved | denied | already_granted",
  "approved_by": "user_prompt"
}
```
