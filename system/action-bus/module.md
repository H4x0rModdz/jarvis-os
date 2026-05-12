# Action Bus

## Purpose

The Jarvis Action Bus (JAB) is the single orchestration layer for every
system interaction on Jarvis OS. Lilith, the shell, automations, and any
future SDK app dispatch through here — there is no other supported path
to "do something" on the system.

## Boundaries

- The bus **dispatches** — it does not implement domain logic. Each
  action's actual work lives in its handler module (`handlers/<ns>.rs`).
- The bus **does not** decide permissions on its own. It calls the
  `Permission System` daemon before every dispatch and obeys the verdict.
- Apps and Lilith **do not** call DBus services directly to bypass the
  bus. Doing so skips permission gating and audit — every dispatch is
  logged to `~/.jarvis/logs/action-bus.log` (JSON Lines).

## Interface

```
DBus  com.jarvis.ActionBus  at  /com/jarvis/ActionBus

  Dispatch(action_json: string) -> string   // JSON response
  ListActions() -> array<string>            // capability discovery
```

`action_json` is the standard request envelope:

```json
{
  "action": "browser.open",
  "params": { "url": "https://example.com" },
  "caller": { "type": "lilith", "id": "lilith" }
}
```

The response always includes `action`, `status`, `duration_ms`, and
either `result` (success) or `error` (failure). See `src/action.rs` for
the exact shape.

## Action Catalog

28 actions registered. The ones backed by real handlers are usable from
day one; the stubs return `UNAVAILABLE` so callers don't silently
no-op.

| Namespace | Actions | Status |
|---|---|---|
| `app.*`        | `open`, `close`                                      | ✅ working (xdg-open / pkill) |
| `app.*`        | `install`, `uninstall`                               | ⏸ stub — needs compatibility layer |
| `file.*`       | `move`, `copy`, `delete`                             | ✅ working (`gio trash` by default; `permanent: true` skips the trash) |
| `window.*`     | `focus`, `minimize`, `maximize`, `close`, `move`, `resize`, `snap_left`, `snap_right` | ⏸ stub — the Jarvis compositor (Phase 3) will register real handlers |
| `workspace.*`  | `switch`, `move_window`, `create`                    | ⏸ stub — same as above |
| `system.*`     | `notify`                                             | ✅ working (notify-send) |
| `system.*`     | `set_setting`, `get_setting`                         | ✅ working (DBus client of `com.jarvis.Settings`) |
| `browser.*`    | `open`                                               | ✅ working (xdg-open, http/https/mailto only) |
| `clipboard.*`  | `set`, `get`                                         | ✅ working (wl-clipboard with xclip fallback) |
| `screenshot.*` | `capture`                                            | ✅ working (grim/scrot, region mode via slurp) |
| `audio.*`      | `set_volume`, `adjust_volume`, `toggle_mute`         | ✅ working (pactl → PipeWire/PulseAudio) |
| `voice.*`      | —                                                    | ⏸ planned (Phase 2 voice pipeline) |

## Permission Flow

```
caller → Dispatch(request)
            ↓
       Permission.Check(caller, scope, action)
            ↓ approved
       handler.run(params)
            ↓
       audit log
            ↓
       response to caller
```

If the Permission daemon is unreachable, the bus falls back to a local
deny-by-default policy on dangerous scopes; safe scopes stay allowed so
the system doesn't brick itself when the daemon hiccups. See
`src/permission.rs`.

## Performance Characteristics

- Startup time: < 50 ms
- Dispatch overhead: ~1 ms for the bus itself (the handler is whatever
  the underlying shell-out costs)
- Memory: < 8 MB idle
- Latency-sensitive: yes — sits in every user/AI interaction path

## Known Limitations

- Window and workspace actions return `UNAVAILABLE` until the Jarvis
  compositor lands and registers its own handlers (the registry supports
  per-handler override).
- `app.install` / `app.uninstall` are stubbed pending the compatibility
  layer (Wine/Proton + Flatpak bridge).
- Async dispatch (fire-and-forget with `ActionCompleted` signal) is
  designed in the schema but not implemented.
