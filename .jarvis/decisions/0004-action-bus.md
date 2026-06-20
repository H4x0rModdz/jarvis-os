# ADR 0004: Centralized Action Bus for System Interactions

## Status
Accepted

## Context

LilithOS needs a mechanism for AI, users, and automations to interact with the system. The options were:
1. Direct function calls between components
2. Event system with pub/sub
3. Centralized action bus with structured schemas
4. REST-style API over HTTP/Unix socket

## Decision

Implement a centralized Jarvis Action Bus (JAB) where all interactions resolve into structured, named actions dispatched through a single mediation layer.

## Reasons

- AI compatibility: structured JSON schemas are directly parseable by LLMs
- Auditability: every action passes through one point, enabling complete audit logging
- Permission enforcement: one place to check permissions for all operations
- Automation-friendly: automations and scripts use the same API as AI and users
- Debuggability: observe all system interactions in one log stream
- SDK surface: third-party apps expose capabilities via the same action system

## Consequences

- All system operations are slightly slower due to Action Bus mediation (~1ms overhead)
- New capabilities require Action Bus registration before AI can use them
- The Action Bus becomes a critical component — crashes here affect the whole system (mitigated by process isolation and fast restart)
- Developers must define action schemas for new capabilities

## Alternatives Considered

- **Direct DBus calls everywhere**: No unified permission layer, harder to audit, no structured schema
- **Pub/sub event system**: Good for notifications, bad for request/response operations requiring permissions
- **HTTP REST over localhost**: Too heavyweight, wrong abstraction for a system-level IPC
