# API Patterns

## Action Bus API Pattern

All system interactions follow this schema:

### Request
```json
{
  "action": "verb.noun",
  "caller": "lilith | user | automation:<id> | app:<id>",
  "params": {},
  "session_id": "uuid",
  "idempotency_key": "optional-uuid"
}
```

### Response
```json
{
  "action": "verb.noun",
  "status": "success | error | pending | cancelled",
  "result": {},
  "error": {
    "code": "PERMISSION_DENIED | NOT_FOUND | INVALID_PARAMS | ...",
    "message": "Human-readable description"
  },
  "duration_ms": 42
}
```

## Error Codes

```
PERMISSION_DENIED    ← caller lacks required scope
NOT_FOUND            ← target resource doesn't exist
INVALID_PARAMS       ← schema validation failure
USER_CANCELLED       ← user declined confirmation dialog
ALREADY_EXISTS       ← resource conflict
UNAVAILABLE          ← service temporarily unavailable
TIMEOUT              ← operation exceeded time limit
INTERNAL_ERROR       ← unexpected failure (log + report)
```

## DBus API Pattern

Interface definition format (XML):

```xml
<interface name="com.jarvis.ActionBus">
  <method name="Dispatch">
    <arg name="action" type="s" direction="in"/>
    <arg name="result" type="s" direction="out"/>
  </method>
  <signal name="ActionCompleted">
    <arg name="action_id" type="s"/>
    <arg name="result" type="s"/>
  </signal>
</interface>
```

Rules:
- All methods return immediately or signal async completion
- Never block the DBus thread with long operations
- Use signals for events, not polling

## SDK API Pattern

Public SDK functions must follow:

```rust
// Clear verb_noun naming
// Explicit error return (no panics at public boundary)
// Minimal parameter surface

pub fn install_package(source: PackageSource, opts: InstallOptions) -> Result<PackageId, InstallError>
```

Avoid:
- Functions with more than 4 parameters (use an options struct)
- Returning `Option<T>` when an error type is more informative
- `unwrap()` at any public API boundary
- Generic names: `process()`, `handle()`, `run()`

## Configuration API Pattern

All config is TOML. Reading config:

```rust
// Load and validate at startup
// Fail fast on invalid config — do not silently ignore
// Provide sensible defaults for all optional fields
let config = Config::load_from("/etc/jarvis/system.toml")?;
```

Config files must have a schema document in `standards/`.

## Versioning

- Action Bus actions are versioned: `action_name@2` if breaking change
- DBus interfaces use version suffix: `com.jarvis.ActionBus.V2`
- SDK types use Rust semver: `0.x.y` = unstable, `1.x.y` = stable
