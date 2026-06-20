# Action Bus

## Purpose

The Jarvis Action Bus (JAB) is the single orchestration layer for every
system interaction on LilithOS. Lilith, the shell, automations, and any
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

36 actions registered (built-ins) + any SDK-app actions picked up at
startup. The ones backed by real handlers are usable from day one;
stubs return `UNAVAILABLE` so callers don't silently no-op.

| Namespace | Actions | Status |
|---|---|---|
| `app.*`        | `open`, `close`                                      | ✅ working (xdg-open / pkill) |
| `app.*`        | `install`, `uninstall`                               | ✅ working (Flatpak/Flathub, `--user` install) |
| `file.*`       | `move`, `copy`, `delete`                             | ✅ working (`gio trash` by default; `permanent: true` skips the trash) |
| `window.*`     | `focus`, `minimize`, `maximize`, `close`             | ✅ working on labwc — forwarded to `com.jarvis.Shell` (foreign-toplevel), selected by `target` string (ADR 0025) |
| `window.*`     | `move`, `resize`, `snap_left`, `snap_right`          | ⏸ deferred — need compositor geometry control (Smithay) |
| `workspace.*`  | `switch`, `move_window`, `create`                    | ⏸ deferred — labwc exposes no IPC; lands with the Jarvis compositor |
| `system.*`     | `notify`                                             | ✅ working (DBus client of `org.freedesktop.Notifications` — owned by `jarvis-notifications`) |
| `system.*`     | `set_setting`, `get_setting`                         | ✅ working (DBus client of `com.jarvis.Settings`) |
| `browser.*`    | `open`                                               | ✅ working (xdg-open, http/https/mailto only) |
| `clipboard.*`  | `set`, `get`                                         | ✅ working (wl-clipboard with xclip fallback) |
| `screenshot.*` | `capture`                                            | ✅ working (grim/scrot, region mode via slurp) |
| `audio.*`      | `set_volume`, `adjust_volume`, `toggle_mute`         | ✅ working (pactl → PipeWire/PulseAudio) |
| `updater.*`    | `check`, `apply_os`                                  | ✅ working (DBus client of `com.jarvis.Updater`) |
| `compat.*`     | `run_exe`, `run_exe_in`, `create_prefix`, `list_prefixes` | ✅ working (DBus client of `com.jarvis.Compat`, per-app Wine prefixes) |
| `voice.*`      | —                                                    | ⏸ direct daemon DBus today; bus actions land alongside hotword work |

SDK app actions appear here too — every manifest discovered under
`/usr/share/jarvis/apps/` or `~/.local/share/jarvis/apps/`
contributes one row per declared action, dispatched through a
generic DBus proxy to the app's `com.jarvis.app.<id>` service. See
ADR 0011 + [sdk/jarvis-sdk-types/module.md](../../sdk/jarvis-sdk-types/module.md).

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

- `window.{focus,minimize,maximize,close}` work on labwc by forwarding to
  the shell's `com.jarvis.Shell` service (foreign-toplevel, ADR 0025);
  windows are selected by a `target` string ("active" | app name | title
  substring), not a numeric id.
- `window.{move,resize,snap_*}` and all `workspace.*` actions return
  `UNAVAILABLE` — they need compositor-level geometry/workspace control
  that the Jarvis (Smithay) compositor will provide and register here (the
  registry supports per-handler override).
- Async dispatch (fire-and-forget with `ActionCompleted` signal) is
  designed in the schema but not implemented.
