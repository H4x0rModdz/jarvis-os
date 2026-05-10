# Active Problems

> Track hard unsolved problems here. Remove when resolved. Link to relevant ADR when a decision is made.

## Open Problems

### P001 — Compositor Technology Choice
**Problem:** Build compositor on wlroots vs. implement from scratch vs. fork an existing compositor (Hyprland, etc.)

**Tradeoffs:**
- wlroots: fast to start, proven, but opinionated API surface
- From scratch: full control, but 12-18 months of work before usable
- Fork: fastest start, but divergence management is ongoing cost

**Status:** Undecided — needs ADR

---

### P002 — LLM Offline Fallback Quality
**Problem:** When Ollama is unavailable or the model is too small for the task, Lilith's capabilities degrade severely.

**Tradeoffs:**
- Rule-based fallback: reliable but limited capability
- Require minimum hardware spec: simpler but excludes users
- Tiered capability system: complex but graceful

**Status:** Undecided

---

### P003 — AI Memory Privacy Model
**Problem:** Persistent memory makes Lilith more useful but creates a local privacy risk if the device is compromised.

**Options:**
- Plain SQLite: simple, fast, no protection
- Encrypted SQLite (SQLCipher): strong protection, key management complexity
- OS keychain integration: best UX, platform-dependent

**Status:** Undecided

---

### P004 — Base Linux Distribution
**RESOLVED** — See ADR 0005.
Decision: Fedora Atomic, OCI image model (BlueBuild).
Rationale: immutable base + atomic updates + proven Wine/Proton (Bazzite model) + best Wayland ecosystem.
