# jarvis-greeter

## Purpose

The login screen for Jarvis OS. Spawned by `greetd` at boot, runs as
the unprivileged `greeter` user, collects username + secrets, hands a
session-start request back to greetd. After greetd successfully
authenticates and starts the session, the greeter exits — labwc takes
over and the user lands in the desktop they expect.

See ADR 0012 for the rationale (vs. adopting an off-the-shelf greeter).

## Boundaries

- Greeter **does not** touch PAM. greetd does that on its socket;
  the greeter just relays prompts and responses.
- Greeter **does not** know the password. It collects characters in a
  QLineEdit (echoMode=Password) and sends them through to greetd —
  the greeter process never logs, persists, or even keeps them in
  memory after the response leaves the socket.
- Greeter **does not** depend on the shell or any Jarvis daemon. It
  runs *before* the session starts — `com.jarvis.*` services don't
  exist yet. Theme tokens are duplicated locally rather than imported
  from `Jarvis.Shell`.
- Greeter **must** survive every kind of greetd error gracefully:
  socket missing, auth failure, no such user, session-start failure.
  It does not silently exit — the user always sees what happened.

## Interface (with greetd)

```
Unix socket  /run/greetd.sock   (or $GREETD_SOCK)

JSON, length-prefixed (u32 little-endian + payload). Three message
flows the greeter cares about:

  → { "type": "create_session", "username": "jarvis" }
  ← { "type": "auth_message",
       "auth_message_type": "secret",
       "auth_message": "Password: " }
  → { "type": "post_auth_message_response", "response": "<secret>" }
  ← { "type": "success" }              // auth done
       | "auth_message"                 // another prompt
       | "error"                        // bail
  → { "type": "start_session",
       "cmd": ["labwc"],
       "env": ["XDG_SESSION_TYPE=wayland", ...] }
  ← { "type": "success" }              // greetd execs the session
       | "error"
```

The greeter exits with status 0 once greetd answers `success` to
`start_session`. Anything else and the greeter loops back to the
login screen with the error displayed.

## States

```
idle  ──user types & presses Enter──►  creating_session
                                       │
                                       ▼
                                       awaiting_prompt   ◄─── more auth_messages
                                       │
                                       ▼
                                       awaiting_response (UI shows prompt)
                                       │ user types & sends
                                       ▼
                                       starting_session
                                       │
                                       ├─ success → exit(0), greetd starts labwc
                                       └─ error   → reset to idle with error toast
```

## Failure Modes

| Failure | Behavior |
|---|---|
| `/run/greetd.sock` missing | Show "Não foi possível conectar ao greetd"; retry button. |
| `create_session` returns error | Display the greetd-supplied message; clear inputs; back to idle. |
| `post_auth_message_response` returns error | Same as above. Most commonly bad password. |
| `start_session` returns error | Display the error; back to idle. Rare — usually means labwc isn't installed at the expected path. |
| Greeter window loses focus | Re-grab on every paint — login windows on Wayland are normally fullscreen and exclusive, but labwc lets focus escape on multi-output systems. |

## V1 Scope (this)

- Pre-filled `jarvis` username (single-user). Editable but the V1
  ISO has one human user.
- Password as the only auth method (greetd → PAM → pam_unix).
- Hard-coded session command: `labwc`. Future commits introspect
  `/usr/share/wayland-sessions/` for choice.
- No power buttons, no settings preview, no language picker.
