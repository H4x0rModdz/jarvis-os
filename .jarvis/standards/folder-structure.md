# Folder Structure Standards

## Repository Root

```
jarvis-os/
  .jarvis/              ← AI context, skills, architecture (this folder)
  ai/                   ← AI subsystems
  shell/                ← Desktop environment
  system/               ← Core OS services
  compatibility/        ← Wine, Proton, app compatibility
  sdk/                  ← Developer SDK
  apps/                 ← Native Jarvis applications
  docs/                 ← User and developer documentation
  tools/                ← Build scripts, CI utilities
  .github/              ← GitHub Actions, PR templates
  CLAUDE.md             ← Claude Code project context
  CHANGELOG.md
  README.md
```

## Module Internal Structure

Every module follows this pattern:

```
module_name/
  module.md             ← REQUIRED: module contract
  src/
    lib.rs (or main.rs) ← entry point
    <feature>.rs        ← one file per major feature
  tests/
    unit/
    integration/
  benches/              ← if performance-sensitive
```

## What Goes Where

| Content | Location |
|---|---|
| Business logic | `src/<feature>.rs` |
| Public types/traits | `src/types.rs` or `src/api.rs` |
| Constants | `src/constants.rs` |
| Configuration parsing | `src/config.rs` |
| Error types | `src/error.rs` |
| Test utilities | `tests/helpers/` |

## Prohibited Patterns

```
src/utils.rs          ← forbidden: too vague
src/helpers.rs        ← forbidden: too vague
src/common.rs         ← forbidden: too vague
src/misc.rs           ← forbidden: never
src/temp.rs           ← forbidden: should not exist in main branch
```

## AI Context Folder (.jarvis/)

```
.jarvis/
  skills/               ← behavioral guidelines for AI agents
  architecture/         ← technical architecture documents
  standards/            ← coding and structural standards
  contexts/             ← current project state, goals, problems
  decisions/            ← ADRs (Architecture Decision Records)
  jarvis-core-context.md ← project "bible" — loaded first
```

## Documentation Folder

```
docs/
  architecture/         ← high-level system diagrams
  guides/
    getting-started.md
    contributing.md
    building.md
  rfcs/                 ← RFC proposals
  api/                  ← generated API docs
```

## Rules

- No directory should contain more than ~12 files before considering subdivision
- Subdivision should be by domain/responsibility, never by file type
- Test files live next to the code they test, not in a separate top-level `tests/`
- Build artifacts never enter version control (`.gitignore` enforced)
