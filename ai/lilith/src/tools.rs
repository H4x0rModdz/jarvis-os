use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A tool corresponds 1:1 with an Action Bus action.
///
/// The `schema` field is the OpenAI/Ollama-style function description that gets
/// sent to the LLM so it can decide when to call this tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub action: &'static str,
    pub description: &'static str,
    pub schema: Value,
}

/// A parsed tool invocation, ready to dispatch to the Action Bus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub action: String,
    pub params: Value,
}

/// The full set of tools Lilith exposes, mirroring the Action Bus registry.
pub fn all_tools() -> Vec<Tool> {
    vec![
        // ── apps ────────────────────────────────────────────────────────
        Tool {
            action: "app.open",
            description: "Launch an application by name or .desktop ID.",
            schema: json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string", "description": "Application name, command, or .desktop ID" }
                },
                "required": ["app"]
            }),
        },
        Tool {
            action: "app.close",
            description: "Terminate a running application.",
            schema: json!({
                "type": "object",
                "properties": {
                    "app": { "type": "string" },
                    "force": { "type": "boolean", "description": "Use SIGKILL instead of SIGTERM" }
                },
                "required": ["app"]
            }),
        },
        Tool {
            action: "app.install",
            description: "Install an application package.",
            schema: json!({
                "type": "object",
                "properties": { "package": { "type": "string" } },
                "required": ["package"]
            }),
        },
        Tool {
            action: "app.uninstall",
            description: "Uninstall an application package.",
            schema: json!({
                "type": "object",
                "properties": { "package": { "type": "string" } },
                "required": ["package"]
            }),
        },
        // ── files ───────────────────────────────────────────────────────
        Tool {
            action: "file.move",
            description: "Move a file or directory.",
            schema: json!({
                "type": "object",
                "properties": {
                    "source": { "type": "string" },
                    "destination": { "type": "string" }
                },
                "required": ["source", "destination"]
            }),
        },
        Tool {
            action: "file.copy",
            description: "Copy a file.",
            schema: json!({
                "type": "object",
                "properties": {
                    "source": { "type": "string" },
                    "destination": { "type": "string" }
                },
                "required": ["source", "destination"]
            }),
        },
        Tool {
            action: "file.delete",
            description: "Move a file to trash (or permanent delete if permanent=true).",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "permanent": { "type": "boolean" }
                },
                "required": ["path"]
            }),
        },
        // ── windows ─────────────────────────────────────────────────────
        Tool {
            action: "window.focus",
            description: "Bring a window to the front and focus it.",
            schema: window_id_schema(),
        },
        Tool {
            action: "window.minimize",
            description: "Minimize a window.",
            schema: window_id_schema(),
        },
        Tool {
            action: "window.maximize",
            description: "Maximize a window.",
            schema: window_id_schema(),
        },
        Tool {
            action: "window.close",
            description: "Close a window. If force=true, kill the owning process.",
            schema: json!({
                "type": "object",
                "properties": {
                    "window_id": { "type": "integer" },
                    "force": { "type": "boolean" }
                },
                "required": ["window_id"]
            }),
        },
        Tool {
            action: "window.move",
            description: "Move a window to (x, y) in screen coordinates.",
            schema: json!({
                "type": "object",
                "properties": {
                    "window_id": { "type": "integer" },
                    "x": { "type": "integer" },
                    "y": { "type": "integer" }
                },
                "required": ["window_id", "x", "y"]
            }),
        },
        Tool {
            action: "window.resize",
            description: "Resize a window to width x height pixels.",
            schema: json!({
                "type": "object",
                "properties": {
                    "window_id": { "type": "integer" },
                    "width": { "type": "integer" },
                    "height": { "type": "integer" }
                },
                "required": ["window_id", "width", "height"]
            }),
        },
        Tool {
            action: "window.snap_left",
            description: "Snap a window to the left half of the screen.",
            schema: window_id_schema(),
        },
        Tool {
            action: "window.snap_right",
            description: "Snap a window to the right half of the screen.",
            schema: window_id_schema(),
        },
        // ── workspaces ──────────────────────────────────────────────────
        Tool {
            action: "workspace.switch",
            description: "Switch to a workspace by index.",
            schema: json!({
                "type": "object",
                "properties": { "index": { "type": "integer" } },
                "required": ["index"]
            }),
        },
        Tool {
            action: "workspace.move_window",
            description: "Move a window to another workspace.",
            schema: json!({
                "type": "object",
                "properties": {
                    "window_id": { "type": "integer" },
                    "workspace": { "type": "integer" }
                },
                "required": ["window_id", "workspace"]
            }),
        },
        Tool {
            action: "workspace.create",
            description: "Create a new workspace.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        // ── system ──────────────────────────────────────────────────────
        Tool {
            action: "system.notify",
            description: "Show a desktop notification.",
            schema: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "body": { "type": "string" },
                    "urgency": { "type": "string", "enum": ["low", "normal", "critical"] },
                    "icon": { "type": "string" }
                },
                "required": ["body"]
            }),
        },
        Tool {
            action: "system.set_setting",
            description: "Modify a system setting.",
            schema: json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string" },
                    "value": {}
                },
                "required": ["key", "value"]
            }),
        },
        Tool {
            action: "system.get_setting",
            description: "Read a system setting.",
            schema: json!({
                "type": "object",
                "properties": { "key": { "type": "string" } },
                "required": ["key"]
            }),
        },
        // ── memory (Lilith-internal — bypasses Action Bus) ─────────────
        Tool {
            action: "memory.remember",
            description: "Save a personal fact for future recall. Use for user preferences, \
                          names, settings, anything they want me to remember.",
            schema: json!({
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Short label for the fact, e.g. 'favorite editor'" },
                    "value": { "type": "string", "description": "The fact itself, e.g. 'vscode'" }
                },
                "required": ["key", "value"]
            }),
        },
        Tool {
            action: "memory.recall",
            description: "Retrieve a previously-remembered fact by key.",
            schema: json!({
                "type": "object",
                "properties": { "key": { "type": "string" } },
                "required": ["key"]
            }),
        },
        Tool {
            action: "memory.forget",
            description: "Delete a remembered fact.",
            schema: json!({
                "type": "object",
                "properties": { "key": { "type": "string" } },
                "required": ["key"]
            }),
        },
    ]
}

fn window_id_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "window_id": { "type": "integer" } },
        "required": ["window_id"]
    })
}

/// Format tools for Ollama's `/api/chat` `tools` array (OpenAI-compatible shape).
pub fn ollama_tools_payload(tools: &[Tool]) -> Value {
    let arr: Vec<Value> = tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.action,
                    "description": t.description,
                    "parameters": t.schema
                }
            })
        })
        .collect();
    Value::Array(arr)
}
