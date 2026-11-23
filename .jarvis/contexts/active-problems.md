# Active Problems

> Track hard unsolved problems here. Remove when resolved. Link to relevant ADR when a decision is made.

## Open Problems

### P001 — Compositor Technology Choice
**Problem:** Build compositor on wlroots vs. implement from scratch vs. fork an existing compositor (Hyprland, etc.)

**Tradeoffs:**
- wlroots: fast to start, proven, but opinionated API surface
- From scratch: full control, but 12-18 months of work before usable
- Fork: fastest start, but divergence management is ongoing cost

**Status:** decided — ADR 0036: Hyprland, labwc as fallback (untested on hardware). A custom compositor is
research, not the route to daily-driver features. Still open: which of
wayfire / niri / KWin (if any) replaces labwc for real backdrop blur and
better NVIDIA behaviour — decidable only on GPU hardware.

---

### P002 — LLM Offline Fallback Quality
**RESOLVED** — See ADR 0027.
Decision: the rule-based intent parser handles direct commands fully offline,
and when Ollama is unreachable Lilith returns an honest "modelo de IA offline —
comandos diretos ainda funcionam" message instead of a misleading "não entendi".
Graceful degradation is treated as a UX contract. (Tiered LLM capability by
hardware tier remains a future nicety, not a blocker.)

---

### P003 — AI Memory Privacy Model
**RESOLVED** — See ADR 0027.
Decision: the at-rest boundary is LUKS full-disk encryption (installer), NOT
app-level SQLCipher — on an autologin single-user box the key would live
locally, making app-level crypto security theatre. Lilith's SQLite stores
(facts.db, lilith.db) are chmod 0600 + parent 0700 as defense-in-depth.

---

### P004 — Base Linux Distribution
**RESOLVED** — See ADR 0005.
Decision: Fedora Atomic, OCI image model (BlueBuild).
Rationale: immutable base + atomic updates + proven Wine/Proton (Bazzite model) + best Wayland ecosystem.

---

### P005 — Frame pacing on real hardware (NVIDIA + labwc)
**Problem:** on real hardware (RTX 3060, ultrawide) the shell stuttered even
after rendering moved to the GPU.

**What is fixed (in our code):** a `playerctl` call that blocked the GUI thread
every 2 s; HUD sampling (incl. the `/proc` walk) on the GUI thread; one blanket
stats signal that re-evaluated every HUD binding and rebuilt both Repeaters each
second; a layer + multi-pass blur re-run on every change inside any glass panel;
always-on polling (`pactl`, Lilith `ListFacts`); per-core bars and gauge needles
that re-animated on every 1 Hz sample. Measured under software rendering
(headless labwc, 2560x1080, all three surfaces): the idle shell went from 43%
of a core to 2%. Rules to keep it that way are
in `shell/jarvis-shell/module.md` § Performance rules.

**What is still unverified:** whether any stutter remains that is *not* ours —
i.e. wlroots/NVIDIA frame pacing, or three `QQuickWindow`s sharing a process.
These can only be answered on the machine. The levers are in
`/etc/jarvis/session.env` (no image rebuild): `WLR_RENDERER=vulkan`,
`WLR_NO_HARDWARE_CURSORS=0`, `QSG_RENDER_LOOP=basic`,
`JARVIS_SHELL_SURFACES=…`.

**Status:** open until measured on-device after the fixes above ship.
