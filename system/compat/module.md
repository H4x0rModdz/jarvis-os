# Jarvis Compat

## Purpose

Owns Wine on Jarvis OS. Spawns Windows binaries with a controlled
environment so the rest of the system has a single, predictable
surface for "run this .exe" instead of every app reinventing the
prefix dance.

V1 ships one method (`RunExe`) and one shared prefix. See ADR 0013
for what V2 adds and why we deliberately don't ship it now.

## Boundaries

- Compat **owns** the Wine prefix at `~/.jarvis/wine/default/`. It
  creates it on first run with a sensible `WINEARCH=win64` default
  and never reaches into the user's regular `~/.wine/`.
- Compat **does not** dictate UI. The spawned program draws into the
  user's Wayland session like any other client — labwc handles the
  window. Phase 4+ may add UX (start-menu integration, .lnk parsing),
  but V1 leaves that to whoever calls `RunExe`.
- Compat **does not** approve the call. The Action Bus gates
  `compat.run` through the Permission System; the daemon trusts
  whatever lands on its socket (matching the Permission / Updater /
  Voice posture).

## Interface

```
DBus  com.jarvis.Compat  at  /com/jarvis/Compat

  RunExe(path: string, args: array<string>) -> string  // JSON
       └─ { started: bool, pid?: u32, reason?: string }

  signal ProcessExited(pid: u32, status: i32)
       └─ V1 emits when our spawned child terminates; consumers
          can use this for "your app closed" toasts.
```

Future V2 surface:
```
  CreatePrefix(name: string) -> string
  ListPrefixes() -> string
  InstallApp(source: string, prefix: string) -> string
  ListApps() -> string
```

## Behavior

| Trigger | Action |
|---|---|
| First call ever | Creates `~/.jarvis/wine/default/` via `wineboot --init` on the spawn, blocking until the prefix is ready. ~10 s the first time. |
| Subsequent calls | `wine <path> <args…>` with `WINEPREFIX=~/.jarvis/wine/default/`. Inherits `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR` so the GUI lands in the user's session. |
| Path missing | Returns `started: false, reason: "binary not found"`; doesn't touch wine. |
| Wine not installed | `started: false, reason: "wine not installed"` — the daemon doesn't pre-check on startup so a system that hasn't pulled wine still boots cleanly. |
| Crash mid-run | `ProcessExited(pid, signal)` signal fires; no auto-restart (the user's Word doc isn't ours to relaunch). |

## Failure Modes

| Failure | Behavior |
|---|---|
| `~/.jarvis/wine/default/` corrupted | `wine` errors; the daemon surfaces the stderr verbatim in `reason`. Future `compat.recreate_prefix` lets the user wipe + rebuild. |
| Concurrent calls during prefix init | The second call waits for the first's `wineboot` to finish (serialised by a tokio Mutex around prefix bring-up). |
| Wine prints to stderr but exits 0 | We treat the exit code as truth — wine warnings during normal operation are noisy and don't indicate failure. |
