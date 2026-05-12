# jarvis-sdk-types

## Purpose

The data types and manifest loader shared between the Action Bus
(which discovers SDK apps on startup) and any future SDK tooling
(an `jarvis-app new` scaffold, a `jarvis-app validate` linter,
`jarvis-app install` shipping bits into the well-known dirs).

This is a **pure-data** crate: no DBus, no runtime, no async. It
defines `Manifest`, `Action`, and `load_manifests()`. Everything that
*runs* against an SDK app — actually invoking it, proxying calls,
auto-activating — lives in the Action Bus.

## Manifest Shape

`manifest.toml` (TOML, not JSON — comments allowed, easier to hand-write):

```toml
[app]
id          = "sdk_hello"          # required; must match the directory name
                                    # and the DBus service suffix
name        = "Hello SDK"
version     = "0.1.0"
description = "Smallest possible Jarvis SDK app."
# Optional binary the system can use to auto-launch this app's DBus
# service when its actions are dispatched. Recommended; without it the
# user has to start the app themselves before its actions resolve.
exec        = "/usr/bin/sdk-hello"

[[actions]]
name        = "sdk_hello.echo"     # required; must start with `<app.id>.`
description = "Repeat the message back."
[actions.schema]
type       = "object"
properties = { message = { type = "string" } }
required   = ["message"]
```

`actions.schema` is verbatim JSON Schema, included in `ListActions()`
output so Lilith can decide when to call it.

## Discovery Paths

In priority order (later paths shadow earlier ones for the same id):

1. `/usr/share/jarvis/apps/<id>/manifest.toml` — system-installed.
2. `~/.local/share/jarvis/apps/<id>/manifest.toml` — per-user, takes
   precedence over the system copy.

Future:
3. `$XDG_DATA_DIRS/jarvis/apps/<id>/manifest.toml` — for flatpak.

## Validation Rules

- `app.id` must be present, non-empty, and match `^[a-z][a-z0-9_]*$`.
- The directory name on disk must equal `app.id` — keeps reverse
  lookup trivial.
- Every action's `name` must start with `<app.id>.` so apps can't
  claim built-in or other-app namespaces. Action names also match
  `^[a-z][a-z0-9_.]*$`.
- `actions.schema.type` must be `"object"` — JSON Schema allows other
  root types but every Action Bus dispatch carries an object of
  parameters, so we narrow the contract.
- Duplicate action names across manifests cause the *later* manifest
  to be rejected (system-installed wins over user when the user copy
  is broken; for distinct apps, the second-discovered app loses on
  conflict).

## DBus Contract

The Action Bus expects an SDK app to expose:

```
DBus  com.jarvis.app.<id>  at  /com/jarvis/app/<id>

  Dispatch(action: string, params_json: string) -> string  // JSON envelope
```

The returned envelope mirrors the Action Bus response shape:
`{ "result": <any> }` on success, `{ "error": { "code": string,
"message": string } }` on failure. SDK apps don't have to know the
exact bus protocol — they just answer `Dispatch` with one of these
two shapes.

DBus auto-activation (`/usr/share/dbus-1/services/com.jarvis.app.<id>.service`)
is strongly recommended so the Action Bus can summon a cold app on
first call; without it the app must be running before its actions
resolve.

## Failure Modes

| Failure | Behavior |
|---|---|
| Manifest TOML malformed | Manifest skipped, warning logged. Other manifests still load. |
| Action name doesn't match `<app.id>.…` | Manifest rejected entirely. Apps can't half-register. |
| App's DBus service unreachable when dispatched | Action Bus returns `UNAVAILABLE`; the app is reported as offline. |
| App `Dispatch` raises a DBus error | Action Bus returns `EXECUTION_FAILED` with the error message. |
| App `Dispatch` returns non-JSON | Action Bus returns `INVALID_RESPONSE`. |
