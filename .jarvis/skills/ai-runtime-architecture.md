# AI Runtime Architecture

## Goal

Define how Lilith (the AI core) operates within Jarvis OS as a first-class system component.

## Architecture Overview

```
User Input (voice/text/action)
        |
   Lilith Core
        |
   Intent Parser
        |
   Tool Selector
        |
   Action Bus  ←→  Permission System
        |
   System Services / Apps
```

## Lilith Core Components

### Intent Parser
- Takes raw input (voice transcript, text command, trigger event)
- Produces structured intent: `{ intent: "open_app", params: { app: "vscode" } }`
- Never executes directly — always routes through Action Bus

### Tool Calling
- Lilith calls tools, not raw system APIs
- Each tool maps to one or more Action Bus actions
- Tools are registered, versioned, and permissioned
- Tool schemas follow JSON Schema format

### Memory System

Three tiers:

1. **Session memory** — current conversation/task context, cleared on session end
2. **Persistent memory** — user preferences, habits, saved workflows (user-inspectable)
3. **System memory** — known app capabilities, registered automations (read-only for Lilith)

All memory is:
- Stored locally by default
- Exportable and deletable by the user
- Never used for training without explicit opt-in

### Orchestration

- Lilith can chain actions into workflows
- Each step in a workflow is independently permissioned
- Failed steps do not auto-retry without user awareness
- Long-running workflows surface progress in the UI

## Action Bus Interface

Lilith communicates with the system exclusively via the Action Bus:

```json
{
  "caller": "lilith",
  "action": "open_app",
  "params": { "app": "vscode" },
  "session_id": "uuid",
  "requires_permission": "app.launch"
}
```

The Action Bus:
- Validates the action schema
- Checks permission grants
- Dispatches to the appropriate service
- Returns result to Lilith
- Logs the transaction

## Model Integration

- Supports local models via Ollama
- Supports API models (Claude, etc.) with user-provided keys
- Model selection is user-configurable per use case
- Voice pipeline uses separate, lighter models (Whisper for STT, Piper for TTS)
- AI inference never blocks the UI thread

## Failure Modes

- Model unavailable → fall back to deterministic rule-based actions only
- Permission denied → inform user, do not retry silently
- Action failed → report clearly, offer alternatives
- Infinite loop detected → break and surface to user immediately
