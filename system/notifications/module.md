# Jarvis Notifications

## Purpose

Owns the `org.freedesktop.Notifications` well-known name on the session
bus. Any Linux app that calls `notify-send` lands here; so do
`system.notify` Action Bus dispatches. The daemon re-emits each
notification on a Jarvis-private interface the shell binds to, and
keeps a small history Lilith can read.

## Boundaries

- Notifications **decides** lifetime and id assignment. The shell
  renders; the daemon owns the source of truth.
- Notifications **does not** filter or drop based on content. The
  shell may choose not to surface a notification (Do-Not-Disturb,
  urgency filtering — both later work), but the daemon records
  everything.
- Notifications **does not** persist to disk in V1. The recent-history
  buffer is in-memory and lost on daemon restart. A future commit can
  back it with the Settings daemon's SQLite if user feedback wants
  durable history.

## Interface

### FreeDesktop compatibility (`org.freedesktop.Notifications`)

```
DBus  org.freedesktop.Notifications  at  /org/freedesktop/Notifications

  Notify(app_name, replaces_id, app_icon, summary, body,
         actions, hints, expire_timeout) -> u32  // id
  CloseNotification(id) -> ()
  GetCapabilities() -> [string]
  GetServerInformation() -> (name, vendor, version, spec_version)

  signal NotificationClosed(id, reason)   // 1=expired 2=dismissed 3=closed-by-call 4=undefined
  signal ActionInvoked(id, action_key)    // V2 only — V1 ignores actions[]
```

V1 capabilities reported: `body`, `body-markup`, `persistence`. Actions
are deliberately *not* in the capability list — V1 ignores the
`actions` parameter.

### Jarvis-private interface (`com.jarvis.Notifications`)

```
DBus  com.jarvis.Notifications  at  /com/jarvis/Notifications

  RecentNotifications(limit: u32) -> string   // JSON [{ id, app, summary, body, urgency, posted_at }, ...]

  signal NotificationPosted(id: u32, app: string, summary: string,
                            body: string, urgency: string)
```

`urgency` collapses the FreeDesktop urgency hint into one of `"low" |
"normal" | "critical"` — defaulting to `"normal"` when the hint is
absent. The shell uses this to colour the toast.

## Storage

V1: ring buffer of the last 64 notifications in RAM. Drops on daemon
restart, deliberately — see ADR 0010.

## Failure Modes

| Failure | Behavior |
|---|---|
| Daemon offline | `notify-send` and `system.notify` calls hang at the DBus layer (FreeDesktop bug-for-bug — the daemon is `Requires=`, so this is a fix-the-session-target situation, not a degraded mode). |
| `summary` empty / null | The daemon assigns an empty summary; the toast UI shows the body alone. Matches what existing notification servers do. |
| Buffer full | Oldest notification is evicted on the next post. No allocation; bounded RAM. |

## V1 vs V2

| Item | V1 | V2 (current) | V3 |
|---|---|---|---|
| Storage | RAM ring buffer | RAM ring buffer | + SQLite via the Settings daemon |
| Actions (buttons) | Ignored | ✅ Full `ActionInvoked` round-trip | — |
| Hints honoured | `urgency` | `urgency` | + `image-data`, `category`, `transient`, `desktop-entry` |
| Drawer UI | None — only toasts | ✅ History list in the shell | + dismiss/clear, group by app |
| Action Bus | `system.notify` posts | `system.notify` posts | + `notifications.read`, `notifications.dismiss` |
