# Module Contracts

## What Is a Module Contract

A module contract is the explicit, documented agreement about what a module does, what it exposes, and what it requires. It is the primary interface between modules and between modules and AI agents.

## Required for Every Module

Every module directory must contain a `module.md` at its root.

### Template

```markdown
# Module: <module_name>

## Purpose
One clear paragraph. What problem does this module solve?
What would break if this module didn't exist?

## Exposes

### Actions (Action Bus)
- `action.name` — brief description
  - Params: `{ param: type }`
  - Returns: `{ field: type }`
  - Permissions: `permission.scope`

### API / Functions
- `function_name(param: Type) -> ReturnType` — brief description

### Events / Signals
- `EventName` — when it fires and what it carries

## Depends On
- `module_name` — why this dependency exists
- External: `library@version` — why

## Permissions Required
- `permission.scope` — why this module needs it

## AI Integration Notes
How Lilith or automation agents interact with this module.
What they can trigger. What they cannot touch.

## Performance Characteristics
- Startup time:
- Memory footprint:
- Latency-sensitive: yes/no

## Known Limitations
What this module deliberately does not handle.
```

## Enforcement

- CI checks for `module.md` presence in any new directory
- PRs adding new modules without `module.md` are blocked
- `module.md` must be updated when public API changes

## Contract Stability

Mark contracts with stability levels:

```markdown
## Stability: Stable
## Stability: Experimental
## Stability: Internal (not for external use)
```

Breaking changes to Stable contracts require an RFC and a MAJOR version bump.

## Inter-Module Communication Rules

Modules communicate through:

1. **Action Bus** — for user/AI-initiated operations
2. **DBus** — for service-to-service IPC
3. **Shared events** — for system-wide state broadcasts
4. **Direct function calls** — only within the same process boundary

Modules never import each other's internal implementation files. Only public contracts.

## What Makes a Bad Contract

- Exposes too much (leaky abstraction)
- Depends on implementation details of another module
- Has no clear single purpose
- Cannot be explained in one sentence
- Changes constantly without versioning
