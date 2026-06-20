# LilithOS — Claude Code Project Context

You are working on **LilithOS**, an AI-native desktop operating system.

Before doing any work, read and internalize the following files in order:

## 1. Project Bible (read first)

- `.jarvis/jarvis-core-context.md` — what LilithOS is and is not

## 2. Skills (behavioral guidelines — always active)

- `.jarvis/skills/jarvis-philosophy.md` — core values and what to avoid
- `.jarvis/skills/anti-bullshit-engineering.md` — no overengineering, ever
- `.jarvis/skills/ai-native-architecture.md` — how to design for AI participation
- `.jarvis/skills/context-oriented-development.md` — naming, structure, readability
- `.jarvis/skills/documentation-standards.md` — what to document and how
- `.jarvis/skills/jarvis-design-language.md` — visual design rules
- `.jarvis/skills/qt-qml-ui.md` — Qt/QML patterns and rules
- `.jarvis/skills/linux-runtime.md` — Wayland, DBus, systemd, PipeWire
- `.jarvis/skills/ai-runtime-architecture.md` — Lilith AI runtime design
- `.jarvis/skills/ai-safety.md` — permission model and safety rules
- `.jarvis/skills/wine-proton-integration.md` — Windows compatibility layer
- `.jarvis/skills/large-scale-monorepo.md` — repo organization and process

## 3. Architecture (read when working on relevant subsystem)

- `.jarvis/architecture/action-bus.md`
- `.jarvis/architecture/ai-runtime.md`
- `.jarvis/architecture/window-system.md`
- `.jarvis/architecture/compositor.md`
- `.jarvis/architecture/permissions.md`
- `.jarvis/architecture/filesystem.md`

## 4. Standards (always apply)

- `.jarvis/standards/naming.md`
- `.jarvis/standards/folder-structure.md`
- `.jarvis/standards/module-contracts.md`
- `.jarvis/standards/api-patterns.md`
- `.jarvis/standards/ui-patterns.md`

## 5. Current Context

- `.jarvis/contexts/current-goals.md` — what we're working on now
- `.jarvis/contexts/roadmap.md` — where we're going
- `.jarvis/contexts/active-problems.md` — open unsolved problems
- `.jarvis/contexts/known-limitations.md` — deliberate out-of-scope items

## 6. Decisions Made (check before re-deciding)

- `.jarvis/decisions/0001-linux-base.md`
- `.jarvis/decisions/0002-qt-over-gtk.md`
- `.jarvis/decisions/0003-wayland-first.md`
- `.jarvis/decisions/0004-action-bus.md`

---

## Rules That Are Always Active

1. Every new module gets a `module.md` — no exceptions
2. No file named `utils`, `helpers`, `misc`, or `common`
3. All AI-triggerable actions go through the Action Bus
4. No dangerous action (delete, terminal.execute) without user confirmation
5. Animations are <= 250ms, ease-out curves, purposeful
6. Glassmorphism: blur 8-20px, opacity 0.6-0.85, never on text-heavy surfaces
7. Every abstraction must justify its existence in one sentence
8. If you can't explain a module in 2 minutes, it's too complex

## When Adding a New Module

1. Create the module directory with `snake_case` name
2. Write `module.md` using the template in `.jarvis/standards/module-contracts.md`
3. Register any new actions in the Action Bus schema
4. Define required permissions explicitly
5. Add the module to the repository structure in `.jarvis/standards/folder-structure.md`

## When Making a Significant Architecture Decision

Create an ADR in `.jarvis/decisions/<NNNN>-<short-title>.md` before implementing.
