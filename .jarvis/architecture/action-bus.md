# Architecture: Jarvis Action Bus (JAB)

## Purpose

The Action Bus is the central orchestration layer of LilithOS. All interactions — from AI commands to user gestures to automation scripts — resolve into structured actions dispatched through the bus.

## Design Goals

- Single, auditable path for all system interactions
- AI-compatible action format (structured JSON schema)
- Permission enforcement at dispatch time
- Observable and loggable by design
- No magic — every action is traceable

## Action Schema

```json
{
  "action": "verb_noun",
  "caller": "lilith | user | automation | sdk",
  "params": {},
  "permissions_required": ["scope.permission"],
  "reversible": true,
  "idempotent": false,
  "session_id": "uuid"
}
```

## Core Actions (Initial Set)

```
app.open            { app: string }
app.close           { app: string, force?: bool }
app.install         { source: string, type: "flatpak|appimage|wine" }
app.uninstall       { app: string }

file.move           { source: path, destination: path }
file.copy           { source: path, destination: path }
file.delete         { path: path, permanent?: bool }

window.focus        { window_id: string }
window.minimize     { window_id: string }
window.maximize     { window_id: string }
window.close        { window_id: string }

system.notify       { title: string, body: string, urgency: low|normal|critical }
system.set_setting  { key: string, value: any }

voice.listen_start  {}
voice.listen_stop   {}
```

## Dispatch Pipeline

```
Caller → Schema Validation → Permission Check → Handler Lookup → Execute → Audit Log → Return Result
```

If any stage fails, the pipeline stops and returns an error. No silent failures.

## Permission Enforcement

The Permission System intercepts every dispatch and checks:

1. Does the caller have the required permission scope?
2. Has the user approved this scope for this caller?
3. Is this a dangerous action requiring explicit confirmation?

Dangerous actions trigger a UI confirmation before proceeding.

## Audit Trail

Every dispatched action writes to the audit log:

```json
{
  "timestamp": "ISO8601",
  "action": "file.delete",
  "caller": "lilith",
  "params": { "path": "/downloads/old.zip", "permanent": false },
  "approved_by": "user_prompt",
  "result": "success",
  "duration_ms": 12
}
```

## Implementation Notes

- IPC mechanism: DBus on `com.jarvis.ActionBus`
- Handler registration: static at startup + dynamic for app SDK registrations
- Action handlers live in their respective service modules, not in the bus itself
- The bus is a router, not a logic layer
