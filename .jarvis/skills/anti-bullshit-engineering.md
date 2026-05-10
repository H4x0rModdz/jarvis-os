# Anti-Bullshit Engineering

## Goal

Prevent unnecessary complexity. Every line of code must justify its existence.

## Principles

- Simplicity over architectural theater
- Readability over abstraction
- Explicitness over magic
- Fewer files when possible
- Avoid premature optimization
- Every abstraction must earn its place

## Avoid

- Useless factories and providers
- Overengineered interfaces for simple operations
- Excessive dependency injection chains
- Deep inheritance hierarchies
- Tiny fragmented files that hold 10 lines each
- Patterns copied from enterprise Java without reason
- Wrapper classes that wrap nothing meaningful
- Abstract base classes with one implementation

## Preferred

```
GOOD: window_manager.rs
BAD:  window_manager_factory_provider_adapter_strategy_service.rs

GOOD: voice_command_router.rs
BAD:  AbstractVoiceCommandProcessingOrchestrationLayer.rs
```

## Rules

- Every abstraction must justify itself with a concrete, observable benefit
- Every layer must have a clear, distinct purpose
- Every module must be explainable in under 2 minutes to a new engineer
- If you can't say what it does in one sentence, it's too complex
- Three similar lines of code are better than a premature abstraction
- No half-finished implementations — ship complete or don't ship

## Red Flags

If you see any of these, stop and reconsider:

- A class with "Manager", "Handler", "Service", "Provider", "Factory", "Strategy" in a chain
- A file with fewer than 20 meaningful lines
- An interface with only one implementation
- A module that only calls another module
- Configuration objects that configure configuration objects
