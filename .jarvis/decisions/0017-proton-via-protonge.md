# ADR 0017: Proton via Proton-GE Direct, Not Steam Runtime Container

## Status
Accepted

## Context

ADR 0013 deferred Proton to Phase 4 because Wine alone covers the
"run a .exe" surface and Proton brings real weight: a ~300 MB
runtime, Steam-runtime container assumptions, DXVK/VKD3D toggles,
gamescope overlays for the Steam Deck experience.

Phase 4 has to decide which Proton path Jarvis OS ships.

## Options Considered

1. **Steam Runtime container.** The official Valve path. Ship the
   `steam-runtime-sniper` container, route Proton through it. Pros:
   officially supported, matches what Steam does. Cons: container
   image alone is ~500 MB, requires its own systemd-nspawn / podman
   plumbing, the runtime is opinionated about FHS layout in ways
   that fight ostree.
2. **Proton-GE direct.** The community fork that bundles its own
   runtime libraries. Pros: single tarball (~300 MB), runs
   self-contained, no container plumbing, what most non-Steam
   users actually run today. Cons: not officially supported by Valve.
3. **System Wine only, no Proton at all.** Defer indefinitely.

## Decision

**Proton-GE direct, fetched on demand by the user, not baked into
the ISO.**

- The compat daemon gains `RunProton(prefix, path, args)`. It expects
  `proton` at `~/.jarvis/proton-ge/proton` (or wherever
  `JARVIS_PROTON_DIR` points).
- The ISO does **not** ship Proton-GE. Image size matters; users
  who want Proton drop the tarball themselves.
- Per-prefix engine is recorded in `.jarvis-meta.json`. Wine and
  Proton prefixes coexist under separate roots
  (`~/.jarvis/wine/<name>/` vs. `~/.jarvis/proton-data/<name>/`)
  because Proton imposes its own compat-data layout (`pfx/`,
  `version`, `tracked_files`).
- Action Bus action `compat.run_proton` mirrors `compat.run_exe_in`
  one-for-one so the call shape is familiar.

## V1 Behaviour When Proton Is Missing

The daemon answers with a clear `started: false, reason:
"proton not installed — expected at /home/<user>/.jarvis/proton-ge
(set JARVIS_PROTON_DIR to override)"`. Lilith forwards the reason
to the user; no auto-download (we are not in the business of
fetching multi-hundred-megabyte binaries without a UI prompt
behind it).

A future `compat.install_proton` action with a real install UI
(progress, version pick, integrity check) is the right place for
auto-fetch. V1's failure message is the polite placeholder.

## Consequences

**Good:**
- ISO size unchanged.
- Wine + Proton coexist without one polluting the other's prefixes.
- Engine choice is per-prefix and visible in `list_prefixes`.
- Re-using the existing compat surface keeps the Lilith tool
  catalog small — one tool per engine, not a parallel API.

**Bad:**
- First-time setup needs a manual Proton-GE drop. Documented in
  the compat module.md.
- Proton-GE is community-maintained; a future breaking change in
  its CLI surface would land on us. The `proton run <exe>` shape
  has been stable for years, so the risk is low.

## Alternatives Considered

- **Auto-download on first RunProton.** Surprising network use, no
  way to pick a version, and bad UX during install (Lilith hangs
  for 5 min downloading 300 MB without progress). Deferred to V2's
  `compat.install_proton` action.
- **bottles / lutris.** Bigger third-party suites. Bottles in
  particular is a Flatpak we could just suggest the user install
  for managed prefixes. Doesn't replace `RunProton` for Lilith /
  Action Bus dispatch.
