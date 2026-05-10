# Large-Scale Monorepo

## Goal

Keep a multi-subsystem repository navigable, consistent, and contributor-friendly as it grows.

## Repository Structure

```
jarvis-os/
  .jarvis/              ← AI context, skills, architecture docs
  ai/
    lilith-core/
    voice-pipeline/
    memory-store/
  shell/
    compositor/
    window-manager/
    taskbar/
  system/
    action-bus/
    permission-system/
    automation-engine/
  compatibility/
    wine-runner/
    proton-integration/
    flatpak-support/
  sdk/
    api/
    bindings/
    examples/
  apps/
    terminal/
    file-manager/
    settings/
    app-store/
  docs/
  tools/
    build/
    scripts/
    ci/
```

## Module Ownership

Every top-level module has:

- An owner (`OWNERS` file or CODEOWNERS entry)
- A `module.md` at its root
- Its own changelog section
- Its own test suite

## RFC Process

Significant changes require an RFC before implementation:

```
docs/rfcs/
  0001-action-bus-design.md
  0002-lilith-memory-architecture.md
  0003-wine-prefix-isolation.md
```

RFC format:
1. Problem statement
2. Proposed solution
3. Alternatives considered
4. Impact on other modules
5. Migration plan (if breaking)

## Dependency Management

- No circular dependencies between modules
- Inter-module communication only through defined interfaces
- External dependencies are pinned to specific versions
- Dependency updates are batched, tested, and reviewed before merging

## Branching Strategy

```
main          ← always stable, always releasable
dev           ← integration branch
feature/<name>
fix/<name>
rfc/<number>
```

Never commit directly to `main`. All merges go through `dev` first.

## CI/CD Requirements

Every PR must pass:

- Build for all target architectures
- Unit tests for changed modules
- Integration tests for affected subsystems
- Lint and formatting checks
- `module.md` presence check (if new module added)

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
