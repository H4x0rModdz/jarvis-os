# ADR 0012: Custom Qt Greeter Instead of an Off-the-Shelf One

## Status
Accepted

## Context

The ISO has shipped autologin via greetd since Phase 1 — convenient
for VM testing, but not a realistic boot path for a desktop OS people
actually use. Phase 3 wants a real login screen.

greetd already mediates the auth path: it speaks PAM behind a JSON
Unix-socket protocol. We need a *greeter* — the UI process greetd
spawns, which collects the user's input and asks greetd to authorise
the session. The greeter runs as a dedicated low-priv user (we'll
create `greeter`), not as the user being logged in.

Three off-the-shelf greeters exist:

- **tuigreet** — TUI-only. Functional but visually disconnected from
  the rest of Jarvis OS.
- **gtkgreet** — GTK. Decent but pulls in the whole GTK stack just
  for one screen.
- **regreet** — Rust + GTK. Same problem.

## Decision

Build `shell/jarvis-greeter/`: a Qt 6 / QML application matching the
Jarvis design language (dark surface, accent purple, glass card),
talking to greetd directly via its documented JSON-over-Unix-socket
protocol.

## Reasons

- **Design coherence.** The shell, the launcher, the approval dialog,
  the updater splash, the settings panel — every other visible surface
  is Qt/QML with the same Theme tokens. Slipping in a GTK greeter
  would be visually jarring on every single boot.
- **Qt stack is already in the ISO.** The shell needs Qt 6 + QML
  anyway; the greeter reuses the runtime, costs only the binary
  (~5 MB stripped). A GTK greeter would *add* a stack we don't
  otherwise carry.
- **The greetd IPC is trivial.** Length-prefixed JSON over a Unix
  socket, three message types. QLocalSocket + QJsonDocument handle
  both pieces. No need for a wrapper crate.
- **Future hooks fit cleanly.** Phase 3.5 will want biometrics
  (`microphone.listen` scope, voiceprint? a webcam? hardware token?).
  Owning the greeter means those hooks land in code we control
  rather than as patches against someone else's project.

## Consequences

- New crate-less Qt CMake project at `shell/jarvis-greeter/`,
  parallel to `shell/jarvis-shell/`. CMake config copies the
  pattern: Qt 6.5+, layer-shell NOT used (the greeter is the only
  window, no need to anchor).
- New `greeter` system user created in the Containerfile so greetd
  has somewhere to spawn the UI as.
- `iso/assets/greetd/config.toml` drops `initial_session = labwc` /
  `user = jarvis` (the autologin path) and gains
  `default_session.command = "jarvis-greeter"` / `user = "greeter"`.
- The greeter, on successful auth, sends greetd a StartSession with
  `cmd = ["labwc"]` and the standard XDG environment. labwc still
  hosts the actual desktop; the greeter just gates entry.

## V1 vs V2

| Item | V1 (this) | V2 |
|---|---|---|
| User selection | Pre-filled "jarvis" (single user) | Picker driven by `/etc/passwd` with UID >= 1000 |
| Auth methods | PAM password only | + voiceprint (microphone scope) and Yubikey/TPM |
| Session selection | Hard-coded labwc | XDG session files picker |
| Power buttons | None | Shutdown / restart / sleep on the lock screen |
| Theming live preview | None | Adopts the user's saved `theme.*` settings before authenticating |

## Alternatives Considered

- **Adopt regreet/gtkgreet and theme it.** Rejected: GTK stack,
  upstream-update churn, can't extend cleanly for future hooks
  (voice unlock).
- **Keep autologin and skip the login screen entirely.** Rejected:
  fine for a kiosk but every multi-user / privacy-conscious user
  expects to see a login. The Jarvis identity is not a single-seat
  product.
- **TUI greeter (tuigreet).** Rejected: the rest of the OS is
  graphical from the first frame; dropping into a text screen for
  login is jarring.
