# AI Safety & Permissions

## Goal

Ensure Lilith and any AI agent operating within LilithOS cannot cause irreversible harm, cannot bypass user intent, and remains fully auditable.

## Core Rule

**No destructive action executes without explicit user approval.**

This is non-negotiable. "Lilith deleted /home by mistake" is a product-ending event.

## Permission Scopes

Every AI capability must be scoped to a specific permission:

```
filesystem.read.<path>
filesystem.write.<path>
filesystem.delete.<path>
app.install
app.uninstall
network.request
terminal.execute
settings.modify
microphone.listen
camera.access
browser.control
```

Permissions are:
- Granted per-session unless explicitly made persistent
- Revocable at any time from the control center
- Logged on every use
- Shown to the user before first use

## Dangerous Action Categories

These require a confirmation dialog — no exceptions:

- Any `delete` or `uninstall` operation
- Any `terminal.execute` with elevated privileges
- Any write to system directories (`/etc`, `/usr`, `/boot`)
- Any network request to an external service
- Any permission grant expansion

## Sandbox Architecture

- AI code execution happens in an isolated namespace
- No direct filesystem access outside granted scope
- Network access is allow-listed, not default-open
- AI cannot fork new privileged processes
- AI actions are mediated through the Action Bus only

## Audit Log

Every AI action must be logged with:

```json
{
  "timestamp": "ISO8601",
  "action": "action_name",
  "params": {},
  "permission_used": "filesystem.write./downloads",
  "user_approved": true,
  "result": "success | failure | cancelled"
}
```

Logs are:
- Stored locally, never sent without consent
- Viewable in the Jarvis control panel
- Retained for 30 days by default (user-configurable)
- Immutable (append-only, no silent edits)

## Approval UX

When Lilith needs permission for a new action:

1. Show what action is requested and why
2. Show what data will be accessed or modified
3. Show if this can be reversed
4. Give the user Clear Allow / Deny / Allow Once options
5. Never auto-approve based on inferred intent alone

## Prohibited

- Silently accessing files outside permission scope
- Executing shell commands without terminal.execute permission
- Installing software without app.install + user confirmation
- Storing data externally without explicit network permission
- Using screen-reading hacks to bypass official APIs
- Accumulating permissions over time without re-confirmation
