# ADR 0007: First-Boot Updater for Heavy AI Assets

## Status
Accepted

## Context

The LilithOS ISO ships without the LLM weights baked in: the qwen3:4b
model alone is ~2.5 GB, and shipping it inside the image would inflate
every download and every atomic update by that amount even when the user
already has the model on disk. Lilith therefore boots into a state where
the daemon is running but cannot answer because no model is loaded.

We need a way to bridge that gap on first boot without leaving the user
staring at an unexplained "Lilith offline" indicator. The user explicitly
asked for a "Windows Update–style" experience: a clearly-presented
"setting up your system" surface that handles the missing pieces with a
progress UI, rather than expecting them to drop into a terminal and run
`ollama pull` themselves.

Beyond model weights, the same surface will eventually handle bootc OS
updates (newer image pushed to the registry) — same mental model, same UX.

## Decision

Introduce a new system module **`system/updater/`** with a Rust daemon
`jarvis-updater` and a Qt splash window inside `jarvis-shell`.

Responsibilities of the daemon:

1. On user session start, check whether each managed asset is present:
   - Ollama model configured by `LILITH_MODEL` (default `qwen3:4b`)
   - Future: bootc image staged update via `bootc upgrade --check`
2. If anything is missing, expose itself on DBus
   (`com.jarvis.Updater` interface) and start fetching.
3. Stream progress as a DBus signal `Progress(stage, percent, message)`
   so the shell's splash can render a live progress bar without polling.
4. Emit `Completed(success, message)` and exit when done; the splash
   dismisses on receipt of `Completed(true, …)`.

Responsibilities of the splash:

1. `jarvis-shell` subscribes to `com.jarvis.Updater` on startup. If the
   service is on the bus, it shows a full-screen window with the Jarvis
   logo, a status line, and a progress bar.
2. The window blocks regular shell input while the update runs — the user
   should not be staring at a half-working Lilith bar until the model is
   in place.
3. Once `Completed(true)` arrives, the splash fades out and the normal
   shell surface takes over.

The module is registered with `jarvis-session.target` but is **not**
required by it — if the daemon is missing or fails, the rest of the
session still comes up. The updater is conceptually advisory.

## Reasons

- **User experience**: matches the mental model people already have from
  Windows / macOS first-boot setup; no terminal required for a working AI.
- **Image size**: keeps the bootable ISO under ~5 GB and atomic updates
  cheap. The 2.5 GB qwen3:4b weights are pulled once on the user's
  machine, then stay in `/var/lib/ollama` across reboots and OS updates.
- **Unified surface for future updates**: the same splash will later
  drive bootc OS upgrades. One pattern, one piece of UI, one DBus
  contract — instead of a separate "system update" dialog later.
- **Decoupling**: a missing or crashed updater does not block the shell
  from coming up. Worst case the user sees Lilith offline, which is the
  same state as before the updater existed.
- **AI hookup**: the updater is on the Action Bus by extension —
  `updater.check` and `updater.apply` will let Lilith answer "are there
  updates?" and "install pending updates" naturally.

## Consequences

- New Rust crate `system/updater` in the workspace and a new binary
  shipped in the OCI image (~3 MB stripped).
- New Qt window in `jarvis-shell` (`UpdaterSplash.qml` + bridge) plus a
  C++ DBus client that subscribes to `com.jarvis.Updater`.
- New systemd user unit `jarvis-updater.service`, wired into
  `jarvis-session.target` as a `Wants=` (not `Requires=`).
- First boot is slow: the user sees the splash for several minutes while
  the model downloads. This is expected and is the whole point of the
  module — without it the slow path was hidden behind a broken-looking
  AI bar.

## Alternatives Considered

- **Bake the model into the ISO.** Rejected: doubles the image size,
  ties model upgrades to OS upgrades, makes atomic rollbacks expensive.
- **Pull silently inside `jarvis-lilith` on first call.** Rejected: the
  user types a question, then nothing happens for 3 minutes — looks
  broken, no progress feedback, can't be cancelled, and Lilith has no
  business doing infrastructure work.
- **Run `ollama pull` from `labwc-autostart`.** Rejected: works, but
  there is no UI surface and no DBus contract for future Lilith /
  Action-Bus integration. Hidden side effects are exactly what the
  Action Bus pattern exists to avoid.
- **Make the updater a one-shot oneshot script.** Rejected: it needs to
  publish progress and be queryable later for "is an update available?".
  A daemon with a DBus surface is the right shape.
