# Large-Scale Monorepo

## Goal

Keep a multi-subsystem repository navigable, consistent, and contributor-friendly as it grows.

## Repository Structure

The canonical, current layout (kept in sync with
`.jarvis/standards/folder-structure.md`):

```
jarvis-os/
  .jarvis/              ← AI context, skills, standards, ADRs
  ai/
    lilith/             ← AI assistant daemon (Rust)
  shell/
    jarvis-shell/       ← bar / dock / launcher / dialogs (Qt6/QML)
    jarvis-greeter/     ← greetd login UI (Qt6/QML)
    jarvis-lock/        ← lock overlay (Qt6/QML)
    compositor/         ← Smithay scaffold (opt-in build)
  system/
    action-bus/         ← central orchestration daemon (Rust)
    permission/         ← scope policy + approval flow (Rust)
    settings/ updater/ voice/ notifications/ compat/ lock/ pam-jarvis/
  sdk/
    jarvis-sdk-types/   ← manifest schema (shared crate)
  tools/                ← build-iso.sh, lock-ctl, voice-ctl, …
  iso/                  ← Containerfile + assets + build.toml
  docs/
```

## Module Ownership

Every top-level module has:

- A `module.md` at its root
- Its own test suite

## Architecture Decisions (ADR — not RFC)

Significant or hard-to-reverse changes require an **ADR before
implementation**. There is no separate RFC process — the ADR is both the
proposal and the record.

```
.jarvis/decisions/
  0001-linux-base.md
  0004-action-bus.md
  0023-ota-updates-via-ghcr.md
  0024-macos-window-management-on-labwc.md
```

ADR format: **Context · Decision · Consequences · Alternatives rejected.**
Check existing ADRs before re-deciding. See `CONTRIBUTING.md`.

## Dependency Management

- No circular dependencies between modules
- Inter-module communication only through defined interfaces
- External dependencies are pinned to specific versions
- Dependency updates are batched, tested, and reviewed before merging

## Branching Strategy

```
main              ← always stable, always releasable; protected; OTA source
feature/<name>    ← a new capability
fix/<name>        ← a bug fix
adr/<NNNN>-<slug> ← an architecture decision
```

`main` is protected — every push builds + publishes the OS image to ghcr
(the OTA channel, ADR 0023), so it must always be releasable. **Nobody
commits to `main` directly, maintainers included.** All changes land via PR
with green CI + one approving review. Maintainers squash-merge. The full
flow lives in `CONTRIBUTING.md`.

## CI/CD Requirements

Every PR must pass (`.github/workflows/ci.yml`):

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace`
- `cargo test` for each crate
- JSON Schema validation for Action Bus action schemas
- `module.md` present for any new module (review-enforced)

## Versioning

- Semantic versioning: `MAJOR.MINOR.PATCH`
- Each module maintains its own version in its manifest
- Breaking API changes require a MAJOR bump and RFC
- Internal changes don't require RFC but do require changelog entry

## Changelog Format

```markdown
## [0.2.0] - 2026-05-10

### Added
- Action Bus v2 with typed action schemas

### Changed
- Lilith memory store now uses SQLite instead of flat files

### Breaking
- `action_bus::dispatch()` signature changed — see migration guide
```
