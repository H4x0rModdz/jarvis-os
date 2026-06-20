# AI-Native Architecture

## Goal

Design every part of LilithOS assuming AI participation from the start — not as an afterthought.

## Principles

- Modularity: every subsystem is independently understandable
- Explicit contracts: every module declares what it does and what it exposes
- Action-driven: all interactions resolve into structured, named actions
- Context-rich: every module is self-documenting for both humans and AI
- Low fragmentation: prefer fewer, larger, cohesive modules over many micro-files

## Rules

- Every feature must be modular and independently operable
- Every public API must be explicit — no hidden behavior
- Avoid abstractions that aren't immediately obvious
- Prioritize legibility over "correct" academic architecture
- Every module must have a `module.md` describing its purpose, interfaces, and AI integration points
- Every action must be documented with its expected input, output, and side effects
- No magic: if it's not obvious what something does by reading it, rename or restructure it

## Module Contract Requirements

Each module must declare:

```markdown
# Module: <name>
## Purpose
## Exposes (public API / actions)
## Depends On
## Permissions Required
## AI Integration Notes
```

## Action Format

All system actions must follow this structure:

```json
{
  "action": "verb_noun",
  "params": {},
  "permissions_required": [],
  "reversible": true
}
```

## What AI-Native Means in Practice

- Folder names are semantic: `voice_command_router/`, not `utils/`
- Files are named after their single responsibility
- State is explicit, never hidden in singletons
- Events are named and typed, not raw signals
- APIs are versioned and documented
