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
