# Documentation Standards

## Goal

Documentation that is useful for humans, AI agents, onboarding, and automation — not just a formality.

## What Gets Documented

### Always Required

- `module.md` for every module
- `README.md` at repository root
- Public API functions (one-line description minimum)
- Architecture Decision Records for significant decisions
- Breaking changes in CHANGELOG

### Required for Complex Systems

- Architecture diagrams for subsystems with 3+ components
- Flow diagrams for multi-step pipelines (voice pipeline, action execution)
- Permission requirement tables for AI-accessible modules

### Never Write

- Comments explaining what the code obviously does
- Docs that duplicate what types and names already communicate
- Docs that will be wrong within a week (implementation details that change)
- Multi-paragraph docstrings for simple functions

## module.md Template

```markdown
# Module: <name>

## Purpose
One paragraph. What this module does and why it exists.

## Exposes
List of public functions, events, or actions with brief descriptions.

## Depends On
Direct dependencies only. No transitive deps.

## Permissions Required
List of permission scopes this module needs.

## AI Integration Notes
How Lilith or other agents interact with this module.
What they can and cannot do through this module.
```

## API Documentation Format

```rust
/// Routes a recognized voice command to the Jarvis Action Bus.
/// Returns None if the command has no registered handler.
pub fn route(command: VoiceCommand) -> Option<Action>
```

That's enough. Don't add paragraphs of explanation to a function whose name and signature already tell the story.

## Architecture Decision Records (ADRs)

File: `decisions/<NNNN>-<short-title>.md`

```markdown
# ADR 0002: Qt over GTK for UI Framework

## Status
Accepted

## Context
We need a UI framework for the Jarvis desktop environment.

## Decision
Use Qt 6 / QML.

## Reasons
- Best animation/GPU support on Linux
- QML is declarative and AI-readable
- Strong Wayland integration
- Rust bindings available (qmetaobject, slint)

## Consequences
- Team needs QML knowledge
- Licensing must be reviewed (LGPL compliance)

## Alternatives Considered
- GTK4: weaker animation support, CSS styling is fragile
- Electron: too heavy, wrong philosophy for an OS shell
```

## Diagram Standards

- Use ASCII diagrams in markdown for simple flows
- Use Mermaid for complex diagrams (renders in GitHub)
- Never embed binary image files for architecture diagrams — use text formats
- Diagrams must be updated when the architecture changes

## Documentation Anti-Patterns

- Docs written after the fact to satisfy a checklist
- Comments that say "// TODO: document this"
- `module.md` files that are empty or say "coming soon"
- API docs that just repeat the function name
- Architecture docs that describe a design that no longer exists
