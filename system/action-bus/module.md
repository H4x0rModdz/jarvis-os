# Module: action-bus

## Purpose

The Jarvis Action Bus (JAB) is the single orchestration layer for all system interactions.
Every operation — from Lilith AI commands to user gestures to automation scripts — dispatches
through the bus. It is the contract layer between callers and system capabilities.

## Exposes

### DBus Interface
- `com.jarvis.ActionBus` at `/com/jarvis/ActionBus`
- Method: `Dispatch(action_json: String) -> String` — synchronous dispatch
- Method: `ListActions() -> Array<String>` — introspection
- Signal: `ActionCompleted(job_id: String, result_json: String)` — async completion

### Action Namespaces
- `app.*` — application lifecycle (open, close, install, uninstall)
- `file.*` — file operations (move, copy, delete)
- `window.*` — window management (focus, minimize, maximize, close)
- `workspace.*` — virtual desktop management
- `system.*` — system operations (notify, set_setting)
- `voice.*` — voice pipeline control

## Depends On

- `permission-system` — for permission scope validation (stubbed until built)
- `audit-log` — JSON Lines file written to `~/.jarvis/logs/action-bus.log`

## Permissions Required

The Action Bus itself requires no permissions. It enforces permissions on behalf of callers.

## AI Integration Notes

Lilith dispatches all system actions through this bus. Lilith cannot call system APIs directly.
Every Lilith action must:
1. Use a registered action name
2. Pass `"caller": { "type": "lilith" }` in the request
3. Receive approval from the permission system before execution

## Performance Characteristics

- Startup time: < 50ms
- Dispatch overhead: < 1ms per action (excluding handler execution)
- Memory footprint: < 8MB idle
- Latency-sensitive: yes — every user/AI interaction passes through here

## Known Limitations

- Permission system is stubbed (always allows) until `permission-system` module is built
- Window actions are stubbed until `compositor` module is built
- `app.install` is stubbed until `compatibility` layer is built
- Async dispatch (fire-and-forget) not yet implemented
