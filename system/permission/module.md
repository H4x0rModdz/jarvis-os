# Permission System

## Purpose

Decides whether a caller (Lilith, an app, an automation) may perform an
action under a given scope. Every Action Bus dispatch consults this
daemon before running the handler.

## Boundaries

- Permission **decides** outcome — it does not execute the action.
- Permission **does not** trust the caller field. It takes it from the
  request as-is. Authenticating callers is the OS bus's job (DBus
  connection identity).
- Action Bus **must** call this daemon before dispatch. If the daemon is
  unreachable, Action Bus falls back to a deny-by-default local policy
  for dangerous scopes; safe scopes stay allowed so the system isn't
  bricked when this daemon is down.

## Interface

```
DBus  com.jarvis.PermissionSystem  at  /com/jarvis/PermissionSystem

  Check(caller: string, scope: string, action: string) -> string  // JSON
       └─ { outcome: "approved"|"denied", approved_by: string }
       └─ for dangerous scopes without an existing grant, blocks up to
          30 s while waiting for the UI to ResolveApproval. On timeout,
          returns denied with approved_by = "approval:timeout".

  ResolveApproval(request_id: string, decision: string) -> string  // JSON
       └─ decision ∈ { "approve", "approve_persistent", "deny" }
       └─ called by the shell when the user clicks a button on the
          approval dialog. Persistent approvals are stored as grants.

  Grant(caller: string, scope: string, persistent: bool) -> string  // JSON
       └─ pre-authorize a (caller, scope) pair. Used by tests and
          headless setups.

  Revoke(caller: string, scope: string) -> string  // JSON
       └─ { revoked: bool }

  ListGrants() -> string  // JSON array

  signal ApprovalRequested(request_id, caller, scope, action)
       └─ emitted when Check() needs the user's call. The shell's
          ApprovalDialog binds to this signal.
```

## Scope Policy

Auto-allowed without prompting (reversible, non-private):

```
app.launch            // launching apps is reversible
window.control        // window mgmt is non-destructive
system.notify         // notifications cannot read state
settings.read         // pure read
filesystem.read       // pure read
audio.control         // volume / mute, reversible by user
clipboard.write       // user can always paste over
```

Always require an explicit grant (destructive or privacy-sensitive):

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
clipboard.read        // can leak passwords / private messages
screen.read           // can leak banking / private info via screenshots
```

The unsafe set matches the "Dangerous Action Categories" in
`.jarvis/skills/ai-safety.md`. Unknown scopes return denied with
`approved_by = "policy:unknown_scope"`.

## Approval UX

When a caller hits a dangerous scope without an existing grant:

1. The daemon emits `ApprovalRequested(request_id, caller, scope, action)`.
2. `jarvis-shell`'s `PermissionBridge` receives the signal and `ApprovalDialog.qml` opens centered on screen.
3. The user clicks one of:
   - **Negar** → `ResolveApproval(id, "deny")` → outcome `denied`.
   - **Permitir uma vez** → `ResolveApproval(id, "approve")` → outcome `approved`, no grant stored.
   - **Permitir sempre** → `ResolveApproval(id, "approve_persistent")` → grant added.
4. If 30 s pass with no resolution, the request auto-denies and the dialog clears.

## Storage

Grants live in an in-memory `HashSet<(caller, scope)>` for Phase 1 — they
do not survive a daemon restart. Phase 2 will back this with SQLite at
`~/.jarvis/permissions.db`. The interface above does not change.

## Failure modes

| Failure | Behavior |
|---|---|
| Caller unknown / malformed | `denied` with `approved_by = "policy:malformed"` |
| Scope not in any known bucket | `denied` with `approved_by = "policy:unknown_scope"` |
| 30 s without user resolution | `denied` with `approved_by = "approval:timeout"` |
| Daemon panics | Action Bus's local fallback kicks in (safe-allow / dangerous-deny) |
