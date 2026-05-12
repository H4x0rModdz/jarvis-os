# Jarvis Updater

## Purpose

Runs once per user session, brings the system to a "ready to use" state
by pulling assets the ISO deliberately did not bake in. Phase 1 scope is
the Lilith model; later phases will also drive bootc OS upgrades from
the same surface.

The user-facing rationale lives in [ADR 0007](../../.jarvis/decisions/0007-first-boot-updater.md).

## Boundaries

- Updater **owns** the decision of *what* needs to be updated and the
  bytes-on-the-wire fetch. It does **not** own the splash UI — that
  lives in `jarvis-shell` and listens to the updater's DBus signals.
- Updater **does not** install OS packages directly with `dnf`. It is a
  bootc system: the only supported OS update path is `bootc upgrade`.
- Updater **does not** speak to the Action Bus internally; it is a leaf
  service. Action Bus will expose `updater.*` actions later that proxy
  to this daemon's DBus interface — that wiring lives in the action bus,
  not here.

## Interface

```
DBus  com.jarvis.Updater  at  /com/jarvis/Updater

  Check() -> string  // JSON
       └─ { model_present: bool, model: string,
            os_update_available: bool|null }

  Apply() -> string  // JSON
       └─ kicks off the pull(s). Returns immediately with
          { started: bool, reason?: string }. Listen to Progress/Completed.

  signal Progress(stage: string, percent: int, message: string)
       └─ stage ∈ { "model.pull", "os.upgrade" }
          percent ∈ [0, 100]; -1 for indeterminate
          message is a short human string ("downloading qwen3:4b…")

  signal Completed(success: bool, message: string)
       └─ fires once per Apply() call. After this the daemon is idle
          and will exit if started by --oneshot.
```

## Behavior

| Trigger | Action |
|---|---|
| User session starts (systemd) | Daemon comes up, calls `Check()` against itself, and if any asset is missing, calls `Apply()` automatically. |
| External `Check()` call | Returns current state without side effects. |
| External `Apply()` while already running | Returns `{ started: false, reason: "busy" }`; no new download starts. |
| Ollama HTTP unreachable | Emits `Completed(false, "ollama unreachable")` and exits. The shell shows a retryable error toast. |
| Network drops mid-pull | Ollama retries internally; updater translates retry events into `Progress(stage, -1, "reconnecting…")`. |

## Implementation Notes

- HTTP client to Ollama is `reqwest` with streaming JSON line parsing
  (`/api/pull` returns NDJSON: each line is a `{status, completed, total}`
  document). Convert byte progress to percent and throttle signals to
  no more than 5/sec to keep DBus traffic sane.
- `OLLAMA_HOST` and `LILITH_MODEL` env vars override the defaults; the
  daemon reads them from its own environment, not from a config file.
- The systemd unit is `jarvis-updater.service`, wired into
  `jarvis-session.target` with `Wants=`. A failed updater never blocks
  the rest of the session — the shell is designed to come up with the
  AI in a "no model" state and degrade gracefully.

## Phase 1 vs Phase 2

| Item | Phase 1 | Phase 2 (current) | Phase 3 |
|---|---|---|---|
| Asset coverage | Ollama model only | + bootc OS upgrade check + apply | + voice models pulled on demand |
| Trigger | session start | + bootc probe on startup | + periodic check (24h timer) |
| AI hookup | none | `updater.check`, `updater.apply_os` on the Action Bus | — |
| Cancellation | none | none | `Cancel()` DBus method |
| History | tracing logs only | tracing logs only | structured update log under `~/.jarvis/logs/updater.log` |

## Failure Modes

| Failure | Behavior |
|---|---|
| Ollama service down | `Completed(false, "ollama daemon not responding")`. User sees a "Lilith offline — open settings to retry" indicator. |
| Disk full during pull | Ollama returns an error in the NDJSON stream; daemon propagates the message. |
| Daemon panics | Splash never receives `Completed` and times out after 10 minutes, falls back to a "setup failed — try again later" view. |
| Model already present | `Check()` returns `model_present: true`; daemon exits cleanly without showing the splash. |
