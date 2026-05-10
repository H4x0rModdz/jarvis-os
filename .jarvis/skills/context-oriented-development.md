# Context-Oriented Development

## Goal

Write code that is predictable, semantically obvious, and easy for both humans and AI to navigate without deep context loading.

## Core Idea

Names should be self-explanatory at a glance. Structure should communicate intent without requiring reading implementation.

## Naming Rules

### Files

```
GOOD: window_animation_engine.rs
GOOD: app_install_service.rs
GOOD: voice_command_router.rs
GOOD: lilith_memory_store.rs

BAD: utils.rs
BAD: helpers.rs
BAD: common.rs
BAD: misc.rs
BAD: manager.rs (alone)
```

### Folders

```
GOOD: ai_runtime/
GOOD: window_compositor/
GOOD: action_bus/
GOOD: voice_pipeline/

BAD: utils/
BAD: helpers/
BAD: common/
BAD: lib/ (unless it's literally a library)
```

### Functions

```
GOOD: route_voice_command_to_action()
GOOD: install_flatpak_package()
GOOD: apply_window_blur_effect()

BAD: process()
BAD: handle()
BAD: do_thing()
BAD: run()
```

## Module Structure

Every module folder must contain a `module.md`:

```markdown
# Module: voice_command_router

## Purpose
Routes recognized voice commands to the appropriate Jarvis Action Bus action.

## Exposes
- route(command: VoiceCommand) -> Action
- list_supported_commands() -> Vec<Command>

## Depends On
- lilith_stt (speech-to-text output)
- action_bus (action dispatching)

## Permissions Required
- microphone_read
- action_dispatch

## AI Integration Notes
Lilith may call route() directly after STT output.
New commands can be registered via the action registry.
```

## Semantic Predictability

A new engineer (or AI agent) reading the repo for the first time should be able to:

1. Understand what a module does from its folder name alone
2. Find the relevant file without searching
3. Read a function signature and know what it does
4. Open `module.md` to get full context in under 2 minutes

If any of these fail, the naming or structure needs improvement.

## Anti-Patterns

- One giant `core.rs` that does everything
- Files named after the developer's mood: `stuff.rs`, `temp.rs`, `new.rs`
- Deeply nested folders with no purpose differentiation
- Context that exists only in the developer's head
