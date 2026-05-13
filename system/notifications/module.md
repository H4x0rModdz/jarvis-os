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
- Notifications **does not** persist to disk yet. The recent-history
  buffer is in-memory and lost on daemon restart. V3 backs it with
  the Settings daemon's SQLite if user feedback wants durable
  history.

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
  signal ActionInvoked(id, action_key)    // fired when the user clicks a button on a toast
```

Capabilities reported: `body`, `body-markup`, `persistence`,
`actions`. The shell renders each `actions[]` entry as a button on
the toast and calls `InvokeAction(id, key)` on
`com.jarvis.Notifications` when the user clicks, which makes the
daemon emit `ActionInvoked(id, key)` for the originating app.

### Jarvis-private interface (`com.jarvis.Notifications`)

```
DBus  com.jarvis.Notifications  at  /com/jarvis/Notifications

  RecentNotifications(limit: u32) -> string   // JSON [{ id, app, summary, body, urgency, posted_at, actions }, ...]
  InvokeAction(id: u32, key: string) -> ()    // shell → daemon when user clicks a toast button

  signal NotificationPosted(id: u32, app: string, summary: string,
                            body: string, urgency: string,
                            actions: array<string>)
```

`urgency` collapses the FreeDesktop urgency hint into one of `"low" |
"normal" | "critical"` — defaulting to `"normal"` when the hint is
absent. The shell uses this to colour the toast.

## Storage

V3 (current): SQLite at `~/.jarvis/notifications.db`. Capped at 500
rows; insertion order tracked monotonically and oldest rows are
evicted when the table grows past the cap. Survives daemon restarts;
`next_id` is seeded from `MAX(id)` on startup so we don't recycle
ids across restarts.

V1 ran in RAM only (64 entries, dropped on restart). V3 keeps the
same shape on the wire — `RecentNotifications` JSON is identical —
just backs it with disk.

## Failure Modes

| Failure | Behavior |
|---|---|
| Daemon offline | `notify-send` and `system.notify` calls hang at the DBus layer (FreeDesktop bug-for-bug — the daemon is `Requires=`, so this is a fix-the-session-target situation, not a degraded mode). |
| `summary` empty / null | The daemon assigns an empty summary; the toast UI shows the body alone. Matches what existing notification servers do. |
| Buffer full | Oldest notification is evicted on the next post. No allocation; bounded RAM. |

## V1 vs V2

| Item | V1 | V2 | V3 (current) | V4 |
|---|---|---|---|---|
| Storage | RAM ring buffer (64) | RAM ring buffer (64) | SQLite at `~/.jarvis/notifications.db` (500 cap) | + sync between devices |
| Actions (buttons) | Ignored | ✅ Full `ActionInvoked` round-trip | unchanged | — |
| Hints honoured | `urgency` | `urgency` | `urgency` | + `image-data`, `category`, `transient`, `desktop-entry` |
| Drawer UI | None — only toasts | ✅ History list | + per-row dismiss + clear all | + group by app, DND policies |
| Action Bus | `system.notify` posts | `system.notify` posts | unchanged | + `notifications.read`, `notifications.dismiss` |
