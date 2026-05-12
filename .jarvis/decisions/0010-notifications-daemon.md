# ADR 0010: Notifications — Daemon-Owned, FreeDesktop-Compatible

## Status
Accepted

## Context

Notifications are a system-wide concept: every Linux app that talks to
libnotify dispatches `Notify(...)` on the well-known DBus name
`org.freedesktop.Notifications`. Whichever process holds that name owns
the user's notification UX.

Our `system.notify` Action Bus action was shelling out to `notify-send`,
which in turn talks to whatever notification server the OS happens to
have (none by default on a labwc + nothing-else session). That works
for our own calls but does nothing for third-party apps, doesn't let
Lilith see what's happening on the desktop, and means we don't control
the visual style of the toasts.

## Decision

Introduce a dedicated `jarvis-notifications` system daemon that:

1. **Acquires `org.freedesktop.Notifications`** on the session bus and
   implements the minimum FreeDesktop spec needed for compatibility:
   `Notify`, `CloseNotification`, `GetCapabilities`,
   `GetServerInformation`, plus the `NotificationClosed` signal.
2. **Re-emits each notification** on our own `com.jarvis.Notifications`
   interface (signal `NotificationPosted(id, app, summary, body,
   urgency)`) so the shell can render a toast without re-implementing
   the spec.
3. **Buffers recent notifications** in memory (last 64) and exposes a
   `RecentNotifications()` method so the shell's future drawer + Lilith
   can both ask "what's pending?" without subscribing to every burst.

The shell hosts the toast UI (Qt window in the bottom-right of the
primary output). Action Bus's `system.notify` routes through the
daemon's DBus interface — `notify-send` is no longer involved.

## Reasons

- **Compatibility with the rest of the Linux ecosystem.** Any app that
  uses libnotify drops into the same UX as Jarvis's own notifications.
  Without owning the well-known name there *is* no UX.
- **Single channel for AI awareness.** Lilith asking "got any messages?"
  becomes a `RecentNotifications()` call instead of a fragile scrape of
  whoever drew the most recent toast.
- **Separation matches the rest of the stack.** Permission, Settings,
  Updater, Voice are all daemons. The shell stays a thin UI process;
  daemons own state and surface it via DBus signals. Notifications
  follow the same shape, including the same "Wants= but not Requires="
  posture so the shell still comes up when the daemon hiccups.
- **No-libnotify path is free.** A future Jarvis SDK app can call our
  own `com.jarvis.Notifications.Post(...)` directly, skipping the
  FreeDesktop compatibility layer and getting the structured fields
  Lilith expects.

## Consequences

- New crate `system/notifications/`, new binary in the OCI image
  (~3 MB stripped).
- New systemd user unit `jarvis-notifications.service` wired into
  `jarvis-session.target` with `Requires=` — without it third-party
  apps' notifications hang at the DBus call, which is a worse failure
  mode than the rest of the session being down.
- `system.notify` Action Bus handler stops shelling out to
  `notify-send`; the dep on `libnotify-bin` leaves the ISO. CI's
  apt-get list drops it too.
- The shell gains a `NotificationsBridge` and a `NotificationToast.qml`
  component. Both bind to the daemon's signals, not the FreeDesktop
  interface directly — the daemon translates.

## V1 vs V2

| Item | V1 (this) | V2 |
|---|---|---|
| Spec methods | Notify, CloseNotification, GetCapabilities, GetServerInformation | + persistent storage |
| Hints | `urgency` only (low / normal / critical) | + `image-data`, `category`, `transient` |
| Actions (buttons) | dropped — `Notify` accepts the array but ignores it | full action support with `ActionInvoked` signal |
| Shell UI | Single toast in the corner, 5 s auto-expire | Toast stack + history drawer + filter by urgency |
| AI hookup | `RecentNotifications()` for Lilith | `notifications.read` + `notifications.dismiss` action bus actions |

## Alternatives Considered

- **Host the spec inside the shell.** Rejected: the shell is a Qt UI
  process. Adding a DBus *service* (rather than client) crosses the
  responsibility line we've held since Phase 1 — daemons own state and
  long-running interfaces; the shell renders.
- **Forward `notify-send` to a real upstream daemon (dunst, mako).**
  Rejected: gives up the AI awareness, the design language control,
  and the single source of truth for "what notifications has the user
  seen?" — all the things this daemon exists to provide.
- **Adopt the Notification Daemon from KDE or GNOME.** Rejected for
  the same reasons as above plus dragging in their dep trees.
