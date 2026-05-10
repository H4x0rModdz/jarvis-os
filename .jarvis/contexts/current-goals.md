# Current Goals

> Update this file as goals evolve. This is the active work context.

## Current Phase

**Phase 1 — Foundation & Architecture**

Establishing the architectural base before writing production code.

## Decisions Locked

- **Base OS:** Fedora Atomic, OCI image model (ADR 0005)
- **UI:** Qt6/QML + Wayland (ADR 0002, 0003)
- **Core IPC:** Jarvis Action Bus via DBus (ADR 0004)
- **Implementation order:** Action Bus → Compositor → Lilith

## Active Goals

1. ~~Define and document the full system architecture~~ ✓
2. ~~Establish the `.jarvis/` context system~~ ✓
3. ~~Choose technology stack and base OS~~ ✓ (ADRs 0001-0005)
4. Design Action Bus schema and IPC contracts (next)
5. Set up monorepo structure with Containerfile base
6. Write module contracts for Action Bus, Permission System, Compositor

## Implementation Queue (B → A → C)

**B — Action Bus ✓ DONE**
- Full JSON Schema (request, response, 5 action namespaces)
- DBus interface XML
- Rust daemon with full dispatch pipeline
- Permission stub + audit log
- 3 unit tests

**A — Compositor (in progress)**
- wlroots-based compositor skeleton
- Basic window rendering
- Wayland shell protocols (xdg-shell, layer-shell)
- Connect to Action Bus for window actions

**C — Lilith (after compositor)**
- Ollama integration (qwen3:4b default model)
- Intent → Action Bus routing
- Basic tool calling (open app, move file, etc.)

## Success Criteria for Phase 1

- Containerfile that builds a bootable Fedora Atomic image
- Action Bus dispatching actions end-to-end with permission checks
- Module contracts written for all Phase 1 modules
- CI pipeline: build image + run Action Bus tests
