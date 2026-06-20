# Jarvis Compat

## Purpose

Owns Wine on LilithOS. Spawns Windows binaries with a controlled
environment so the rest of the system has a single, predictable
surface for "run this .exe" instead of every app reinventing the
prefix dance.

V2 adds per-app prefixes: heavyweight apps (games, Office installs)
get their own `~/.jarvis/wine/<name>/` so they don't pollute the
shared `default` prefix. The V1 `RunExe` entry point still works
and now delegates to a shared body that targets the `default`
prefix. See ADR 0013 for the rationale.

## Boundaries

- Compat **owns** the Wine prefix tree at `~/.jarvis/wine/`. It
  creates each named prefix on first use with `WINEARCH=win64` and
  never reaches into the user's regular `~/.wine/`.
- Compat **does not** dictate UI. The spawned program draws into the
  user's Wayland session like any other client. Phase 4+ may add UX
  (start-menu integration, .lnk parsing); V2 leaves that to whoever
  calls `RunExe` / `RunExeIn`.
- Compat **does not** approve the call. The Action Bus gates
  `compat.run` through the Permission System; the daemon trusts
  whatever lands on its socket (matching the Permission / Updater /
  Voice / Notifications posture).

## Interface

```
DBus  com.jarvis.Compat  at  /com/jarvis/Compat

  RunExe(path: string, args: array<string>) -> string  // JSON
       └─ Default-prefix Wine runner. Same shape as RunExeIn with
          prefix="default" hard-coded.

  RunExeIn(prefix: string, path: string, args: array<string>) -> string  // JSON
       └─ { started: bool, pid?: u32, prefix: string, reason?: string }
          Wine in a named prefix. Creates on first use; concurrent
          first-time calls serialise behind a tokio Mutex.

  RunProton(prefix: string, path: string, args: array<string>) -> string  // JSON
       └─ { started: bool, pid?: u32, prefix: string, engine: "proton", reason?: string }
          Proton-GE in a named prefix at `~/.jarvis/proton-data/<name>/`.
          Returns `reason: "proton not installed — …"` when Proton-GE
          isn't present (see ADR 0017).

  InstallProton() -> string  // JSON
       └─ { ok: bool, already?: bool, version, path?, reason? }
          Downloads + extracts Proton-GE to `~/.jarvis/proton-ge/`.
          Idempotent — returns `already: true` when the binary is
          present. Emits `InstallProgress(percent, message)` signals
          throughout; pushes a single updating toast notification.

  signal InstallProgress(percent: u32, message: string)
       └─ Fires repeatedly during InstallProton. 0..=90 covers the
          download, 90..=100 covers extraction. Subscribers throttle
          themselves by checking `percent` doesn't repeat.

  CreatePrefix(name: string) -> string  // JSON
       └─ { ok: bool, already?: bool, path?: string, reason?: string }
          Pre-creates a Wine prefix so the wineboot --init cost
          lands before the first app launch.

  ListPrefixes() -> string  // JSON
       └─ { prefixes: [{ name, path, initialised, created_at, last_used_at, engine }, …] }
          Lists both Wine and Proton prefixes; same name can appear
          twice (once per engine).

  ListRunning() -> string  // JSON
       └─ { running: [{ pid, prefix, engine, exe, started_at }, …] }
          Snapshot of every child the daemon is currently tracking.

  Terminate(pid: u32) -> string  // JSON
       └─ { ok: bool, reason?: string }
          SIGTERM the tracked child. Refuses pids it doesn't track.

  signal ProcessExited(pid: u32, status: i32)
       └─ Fires when one of our spawned children terminates;
          subscribers use this for "your app closed" toasts.
```

Prefix names must match `^[a-z0-9][a-z0-9_-]*$` (≤ 64 chars). Same
basic shape as Flatpak app IDs and Docker tags — avoids odd shell
quoting in WINEPREFIX paths.

## Behavior

| Trigger | Action |
|---|---|
| First call ever (RunExe) | Creates `~/.jarvis/wine/default/` via `wineboot --init`; blocks until ready (~10 s first time). |
| First call into a new prefix (RunExeIn) | Same as above for `~/.jarvis/wine/<name>/`. |
| Subsequent calls in initialised prefix | `wine <path> <args…>` with WINEPREFIX set. Inherits DISPLAY / WAYLAND_DISPLAY / XDG_RUNTIME_DIR. Per-call last_used_at stamp in `.jarvis-meta.json`. |
| Path missing | `started: false, reason: "binary not found"`. |
| Wine not installed | `started: false, reason: "wine not installed (no \`wine\` in PATH)"`. |
| Crash mid-run | `ProcessExited(pid, signal)` signal; no auto-restart. |

## Metadata Storage

Each prefix gets a tiny `.jarvis-meta.json` next to its
`system.reg`, recording `created_at` (first init) and `last_used_at`
(every successful spawn). `ListPrefixes` surfaces these so future
UIs can show "Steam (last used 3 days ago)" without statting
system.reg's mtime.

## Failure Modes

| Failure | Behavior |
|---|---|
| Prefix corrupted | `wine` errors; daemon surfaces the stderr verbatim. V3 will add `compat.recreate_prefix` so the user can wipe + rebuild from inside the OS. |
| Concurrent first-time calls into same prefix | Second waits for the first's `wineboot` to finish (`prefix_init` tokio Mutex). |
| Concurrent calls into already-initialised prefix | No lock — wine handles its own concurrency. |
| Wine prints stderr but exits 0 | Treated as success. Wine warnings during normal operation are noisy and don't indicate failure. |

## V1 vs V2 vs V3

| Item | V1 | V2 | V3 (current) | V4 |
|---|---|---|---|---|
| Engines | wine | wine | + Proton-GE direct (see ADR 0017) | + Steam Runtime container option |
| Prefix model | shared `default` | + per-app via RunExeIn / CreatePrefix | Wine + Proton roots coexist | — |
| App catalog | none | none | none | metadata + .lnk parsing + start-menu integration |
| DXVK / VKD3D | wine bundled | wine bundled | wine bundled; Proton bundles its own | toggle per-prefix |
| Proton install | — | — | manual drop at `~/.jarvis/proton-ge/` | `compat.install_proton` with progress UI |
| Lifecycle | fire-and-forget spawn | fire-and-forget spawn | fire-and-forget spawn | tracked PIDs, `compat.list_running`, `compat.terminate` |
| Action Bus surface | `compat.run_exe` | + `run_exe_in`, `create_prefix`, `list_prefixes` | + `run_proton` | + install / uninstall / recreate |
