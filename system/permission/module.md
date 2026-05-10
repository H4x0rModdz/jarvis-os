# Permission System

## Purpose

Decides whether a caller (Lilith, an app, an automation) may perform an action with a given scope. Every Action Bus dispatch consults this service before running the handler.

## Boundaries

- Permission System **decides** outcome — it does not execute the action.
- Permission System **does not** trust the caller field — it takes it from the request as-is. Authentication of callers is the OS bus's job (DBus connection identity).
- Action Bus **must** call this daemon before dispatch. If the daemon is unreachable, Action Bus falls back to a deny-by-default local policy for dangerous scopes; safe scopes stay allowed so the system isn't bricked when this daemon is down.

## Interface

```
DBus  com.jarvis.PermissionSystem  at  /com/jarvis/PermissionSystem

  Check(caller: string, scope: string, action: string) -> string  // JSON
       └─ returns { outcome: "approved"|"denied", approved_by: string }

  Grant(caller: string, scope: string, persistent: bool) -> string  // JSON
       └─ pre-authorize a (caller, scope) pair. For headless/test setups.

  Revoke(caller: string, scope: string) -> string  // JSON
       └─ remove an existing grant

  ListGrants() -> string  // JSON array
```

## Scope Policy (Phase 1)

Auto-allowed without prompting:

```
app.launch            // launching apps is reversible
window.control        // window mgmt is non-destructive
system.notify         // notifications cannot read state
settings.read         // pure read
filesystem.read       // pure read
```

Always deny unless an explicit `Grant()` exists:

```
app.install
app.uninstall
filesystem.write
filesystem.delete
settings.modify
terminal.execute
network.request.external
microphone.listen
camera.access
```

The unsafe scopes match the "Dangerous Action Categories" in `.jarvis/skills/ai-safety.md` line 38.

## Phase 1 vs Phase 2

| Item | Phase 1 (this) | Phase 2 |
|---|---|---|
| Grant storage | in-memory `HashSet<(caller, scope)>` | SQLite at `~/.jarvis/permissions.db` per architecture doc |
| Approval UX | none — `Grant()` is the only path | UI prompt via control center, `ApprovalRequested` DBus signal |
| Persistence | lost on restart | persistent grants survive reboot |
| Audit | tracing logs only | full audit log to `~/.jarvis/logs/permission.log` |

## Failure modes

| Failure | Behavior |
|---|---|
| Caller unknown / malformed | `denied` with `approved_by = "policy:malformed"` |
| Scope not in any known bucket | `denied` with `approved_by = "policy:unknown_scope"` |
| Daemon panics | Action Bus's local fallback kicks in (safe-allow / dangerous-deny) |
