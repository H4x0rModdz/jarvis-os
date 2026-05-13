# jarvis-greeter

## Purpose

The login screen for Jarvis OS. Spawned by `greetd` at boot, runs
under `cage` as the unprivileged `greeter` user, collects username +
secrets, hands a session-start request back to greetd. After greetd
successfully authenticates and starts the session, the greeter exits
and labwc takes over.

See [ADR 0012](../../.jarvis/decisions/0012-custom-greeter.md) for
the rationale (vs. adopting an off-the-shelf greeter).

## Boundaries

- Greeter **does not** touch PAM. greetd does that on its socket;
  the greeter just relays prompts and responses.
- Greeter **does not** keep the password. It collects characters in a
  QML TextInput (echoMode=Password) and sends them through to greetd
  — the greeter process never logs, persists, or even keeps them in
  memory after the response leaves the socket.
- Greeter **does not** depend on any Jarvis daemon. It runs *before*
  the session — `com.jarvis.*` services don't exist yet. Theme
  tokens and branding assets are duplicated locally.
- Greeter **must** survive every kind of greetd error gracefully —
  socket missing, auth failure, no such user, session-start failure.
  It never silently exits; the user always sees what happened.

## Interface (with greetd)

```
Unix socket  /run/greetd.sock   (or $GREETD_SOCK)

JSON, length-prefixed (u32 little-endian + payload):

  → { "type": "create_session", "username": "jarvis" }
  ← { "type": "auth_message",
       "auth_message_type": "secret",
       "auth_message": "Password: " }
  → { "type": "post_auth_message_response", "response": "<secret>" }
  ← { "type": "success" }            // auth done
       | "auth_message"               // another prompt
       | "error"                      // bail
  → { "type": "start_session",
       "cmd": ["labwc"],
       "env": ["XDG_SESSION_TYPE=wayland", "XDG_SESSION_DESKTOP=jarvis"] }
  ← { "type": "success" }            // greetd execs the session
       | "error"
```

The greeter exits with status 0 once greetd answers `success` to
`start_session`. Anything else and the greeter falls back to idle
with the error rendered in the bottom toast.

## Three-Mode Architecture (V1.5)

The single-card V1 was replaced by a SwipeView holding three visually
distinct modes. The user can switch any time before submitting — the
auth path through greetd is identical across all three.

| Mode | Header | Distinctives |
|---|---|---|
| 01 · Standard | `STANDARD LOGIN` | Glass card, eye logo, welcome line (click to edit username), password + UNLOCK SYSTEM, Face ID / Voice ID / PIN pills. |
| 02 · Lilith | `LILITH INTERFACE` | Cinematic avatar column, "Good evening, …", conversational input placeholder, decorative voice waveform, suggestion chips. |
| 03 · Focus | `03 · FOCUS` | Minimal — no glass, no glow. Chevron + "JARVIS OS" title, password, UNLOCK. |

Navigation:
- Swipe (touchpad / touch screens)
- `←` / `→` arrow keys
- Mouse wheel anywhere on the screen
- Click the `●○○` indicators

`GreeterState` (QML singleton backed by `QSettings`) remembers the
last user + last selected mode across boots, so the panel opens
where the user left it.

## Branding

The wallpaper and the eye-logo PNG live at the repo root
(`jarvis-op-default-wallpaper.png`, `jarvis-os-default-icon.png`)
and are baked into the binary under `qrc:/branding/` via
`qt_add_resources`. The lock window shares the same prefix so the
visual transition login → desktop → lock stays continuous.

## V1.5 Status (current)

✅ Three-mode SwipeView + indicators + nav
✅ Username editable + persisted (`~/.config/Jarvis/jarvis-greeter.conf`)
✅ Last-mode persistence
✅ Wallpaper + icon PNG assets
✅ Clock top-right, locale-aware
✅ Toast for errors / info (replaces inline per-mode error rows)

## Avatar Pipeline (V1.6)

`qml/components/AnimeAvatar.qml` is the slot. Behaviour:

- If `qrc:/avatar/lilith-{idle,talking,listening}.png` is compiled
  into the binary, the avatar shows the right sprite per state.
- If not (V1 reality — the Lilith character art is V2 work), the
  procedural fallback renders the same vertical glow column with a
  breathing animation tuned to the state.

Drop-in contract for the real assets: 256 × 360 PNG portraits with
transparent backgrounds, single subject centred, named exactly:

```
qrc:/avatar/lilith-idle.png
qrc:/avatar/lilith-talking.png
qrc:/avatar/lilith-listening.png
```

Adding them: extend the `qt_add_resources` block in
`CMakeLists.txt` with a new `PREFIX "/avatar"` group pointing at the
PNG files. The layout doesn't re-flow when sprites arrive — both
paths render at 140 × 180 inside the greeter card.

## True V2 (deferred — needs new infrastructure)

| Item | Blocked on |
|---|---|
| Anime avatar real | Asset pipeline + licensing for the Lilith character. Infrastructure for it ships in V1.6 (see above). |
| Voice / Face / PIN functional | PAM hooks (custom `pam_*.so` modules) |
| Audio-reactive waveform | cpal capture running as the `greeter` user (audio device permission) |
| Adaptive mode switching | Greeter pre-session can't reach the Settings daemon (it isn't up yet) |
| Backdrop blur | Jarvis compositor (labwc doesn't expose a blur protocol) |

## Failure Modes

| Failure | Behavior |
|---|---|
| `/run/greetd.sock` missing | Toast: "Não foi possível conectar ao greetd"; retry on next submit. |
| `create_session` returns error | Toast with greetd's message; clear inputs; back to idle. |
| `post_auth_message_response` returns error | Same as above. Most commonly bad password. |
| `start_session` returns error | Toast with the error; back to idle. Rare — usually labwc missing. |
| Window loses focus on multi-output | We re-grab on every paint. Wayland normally gates this via cage's exclusive-zone. |
