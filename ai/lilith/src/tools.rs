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
            description: "Install a Linux app via Flatpak/Flathub. The user must confirm — \
                          installing software is a privileged action. `app_id` is the \
                          Flatpak identifier (e.g. 'org.mozilla.firefox', 'org.signal.Signal').",
            schema: json!({
                "type": "object",
                "properties": {
                    "app_id": {
                        "type": "string",
                        "description": "Flatpak app id (reverse-DNS form, e.g. org.mozilla.firefox)"
                    }
                },
                "required": ["app_id"]
            }),
        },
        Tool {
            action: "app.uninstall",
            description: "Uninstall a Flatpak app by its app id.",
            schema: json!({
                "type": "object",
                "properties": {
                    "app_id": {
                        "type": "string",
                        "description": "Flatpak app id (reverse-DNS form)"
                    }
                },
                "required": ["app_id"]
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
            description: "Bring a window to the front and focus it. `target` \
                          picks the window: \"active\" (default) for the focused \
                          one, an app name like \"firefox\", or a title substring.",
            schema: window_target_schema(),
        },
        Tool {
            action: "window.minimize",
            description: "Minimize a window. `target`: \"active\" (default), an \
                          app name, or a title substring.",
            schema: window_target_schema(),
        },
        Tool {
            action: "window.maximize",
            description: "Maximize a window. `target`: \"active\" (default), an \
                          app name, or a title substring.",
            schema: window_target_schema(),
        },
        Tool {
            action: "window.close",
            description: "Close a window. `target`: \"active\" (default), an app \
                          name, or a title substring.",
            schema: window_target_schema(),
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
        // ── browser ─────────────────────────────────────────────────────
        Tool {
            action: "browser.open",
            description: "Open a URL in the default browser. Only http://, https://, mailto:.",
            schema: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Full URL including scheme" }
                },
                "required": ["url"]
            }),
        },
        // ── clipboard ───────────────────────────────────────────────────
        Tool {
            action: "clipboard.set",
            description: "Copy text to the system clipboard so the user can paste it.",
            schema: json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "mime": { "type": "string", "description": "Optional, defaults to text/plain" }
                },
                "required": ["text"]
            }),
        },
        Tool {
            action: "clipboard.get",
            description: "Read the current contents of the clipboard. Requires permission.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        // ── screenshot ──────────────────────────────────────────────────
        Tool {
            action: "screenshot.capture",
            description: "Capture a screenshot. Saved to ~/Pictures/Screenshots by default. \
                          Requires permission.",
            schema: json!({
                "type": "object",
                "properties": {
                    "mode": { "type": "string", "enum": ["full", "region"], "description": "Default 'full'. 'region' lets the user drag-select." },
                    "path": { "type": "string", "description": "Absolute path to save to. Optional." }
                }
            }),
        },
        // ── audio ───────────────────────────────────────────────────────
        Tool {
            action: "audio.set_volume",
            description: "Set the system output volume to an absolute percentage [0, 150].",
            schema: json!({
                "type": "object",
                "properties": {
                    "percent": { "type": "integer", "minimum": 0, "maximum": 150 }
                },
                "required": ["percent"]
            }),
        },
        Tool {
            action: "audio.adjust_volume",
            description:
                "Adjust the system output volume by a signed delta percent (e.g. +5, -10).",
            schema: json!({
                "type": "object",
                "properties": {
                    "delta": { "type": "integer", "description": "Signed percent. Positive raises, negative lowers." }
                },
                "required": ["delta"]
            }),
        },
        Tool {
            action: "audio.toggle_mute",
            description: "Mute or unmute the default audio sink. Pass set_state to force a value, \
                          omit it to toggle.",
            schema: json!({
                "type": "object",
                "properties": {
                    "set_state": { "type": "boolean", "description": "true mutes, false unmutes, omit to toggle" }
                }
            }),
        },
        Tool {
            action: "audio.list_sinks",
            description: "List every available audio output sink + which one is currently the \
                          default. Use when the user asks 'que saídas tem' / 'lista as saídas \
                          de áudio' / 'que saídas tô usando'.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            action: "audio.set_default_sink",
            description: "Switch the system's default audio output and migrate any currently \
                          playing streams to follow. Pass the sink's `name` (not the description); \
                          to get the right name, call audio.list_sinks first if you don't \
                          already have it. Use for 'trocar saída pro fone' / 'manda o som pro \
                          headset' / 'volta pro alto-falante'.",
            schema: json!({
                "type": "object",
                "properties": {
                    "sink": { "type": "string", "description": "Sink name from audio.list_sinks (e.g. alsa_output.pci-0000_00_1f.3.analog-stereo)." }
                },
                "required": ["sink"]
            }),
        },
        // ── network (Wi-Fi) ─────────────────────────────────────────────
        Tool {
            action: "network.scan",
            description: "Scan for nearby Wi-Fi networks and return the fresh list. Use when the \
                          user asks 'que redes tem aqui' or 'procurar Wi-Fi'. Returns networks: \
                          [{ssid, signal, security, in_use}].",
            schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            action: "network.list",
            description: "Return the cached Wi-Fi list without rescanning. Faster than scan; use \
                          when the user asks 'qual minha rede' / 'estou conectado em qual' / \
                          'lista as redes'.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            action: "network.connect",
            description: "Connect to a Wi-Fi network by SSID. Pass password for secured networks; \
                          omit / empty string for open networks. Use for 'conecta no <ssid>' / \
                          'me conecta na rede X senha Y'.",
            schema: json!({
                "type": "object",
                "properties": {
                    "ssid":     { "type": "string", "description": "Network SSID (case-sensitive)." },
                    "password": { "type": "string", "description": "WPA/WPA2 password; empty for open networks." }
                },
                "required": ["ssid"]
            }),
        },
        Tool {
            action: "network.disconnect",
            description: "Drop the active Wi-Fi connection. Radio stays on so a reconnect works \
                          immediately. Use for 'desconecta do Wi-Fi'.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            action: "network.set_enabled",
            description: "Toggle the Wi-Fi radio. Use for 'liga o Wi-Fi' / 'desliga o Wi-Fi'.",
            schema: json!({
                "type": "object",
                "properties": {
                    "enabled": { "type": "boolean", "description": "true turns radio on, false off." }
                },
                "required": ["enabled"]
            }),
        },
        // ── bluetooth ───────────────────────────────────────────────────
        Tool {
            action: "bluetooth.scan",
            description: "Run a 10-second Bluetooth discovery + return the paired + nearby list. \
                          Use for 'procurar dispositivos bluetooth' / 'buscar fones'. Returns \
                          { paired: [{mac, name, connected}], nearby: [{mac, name}] }.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            action: "bluetooth.list_paired",
            description: "Return paired devices with current connection state. Faster than scan; \
                          use when the user asks 'que dispositivos tenho pareados' / 'meus fones \
                          estão conectados'.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            action: "bluetooth.pair",
            description: "Pair + trust + connect a Bluetooth device by MAC. Just-works pairing \
                          only — devices that need a passkey return an error. Use for 'parear \
                          AirPods' / 'parear meus fones'. Get the MAC from bluetooth.scan first \
                          if you don't have it.",
            schema: json!({
                "type": "object",
                "properties": {
                    "mac": { "type": "string", "description": "MAC address, AA:BB:CC:DD:EE:FF format (case-insensitive)." }
                },
                "required": ["mac"]
            }),
        },
        Tool {
            action: "bluetooth.connect",
            description: "Reconnect to an already-paired Bluetooth device. Use for 'conecta nos \
                          meus fones' when the device is in pairedDevices but not connected.",
            schema: json!({
                "type": "object",
                "properties": {
                    "mac": { "type": "string" }
                },
                "required": ["mac"]
            }),
        },
        Tool {
            action: "bluetooth.disconnect",
            description: "Drop a Bluetooth connection without unpairing. Use for 'desconecta os \
                          fones' / 'desconecta do <device>'.",
            schema: json!({
                "type": "object",
                "properties": {
                    "mac": { "type": "string" }
                },
                "required": ["mac"]
            }),
        },
        Tool {
            action: "bluetooth.unpair",
            description: "Remove a Bluetooth device's pairing entirely. Use for 'remove o \
                          pareamento' / 'esquece os fones'.",
            schema: json!({
                "type": "object",
                "properties": {
                    "mac": { "type": "string" }
                },
                "required": ["mac"]
            }),
        },
        Tool {
            action: "bluetooth.set_enabled",
            description: "Toggle the Bluetooth radio. Use for 'liga o bluetooth' / 'desliga o \
                          bluetooth'.",
            schema: json!({
                "type": "object",
                "properties": {
                    "enabled": { "type": "boolean" }
                },
                "required": ["enabled"]
            }),
        },
        // ── compat (Windows app runner) ─────────────────────────────────
        Tool {
            action: "compat.run_exe",
            description: "Run a Windows .exe under Wine in the default prefix. The user must \
                          confirm — this action executes arbitrary Windows code.",
            schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute path to the .exe file" },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional argv tail passed verbatim"
                    }
                },
                "required": ["path"]
            }),
        },
        Tool {
            action: "compat.run_exe_in",
            description: "Run a Windows .exe in a named Wine prefix. Same approval semantics as \
                          compat.run_exe. Use this when the user wants a heavyweight app (game / \
                          MS Office / a specific Photoshop install) isolated from the default.",
            schema: json!({
                "type": "object",
                "properties": {
                    "prefix": {
                        "type": "string",
                        "description": "Prefix name (lowercase / digits / _ / -; first char must be alphanumeric)"
                    },
                    "path": { "type": "string" },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["prefix", "path"]
            }),
        },
        Tool {
            action: "compat.create_prefix",
            description: "Create a new named Wine prefix without running anything. Useful before \
                          a heavyweight install — front-loads the wineboot --init cost.",
            schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                },
                "required": ["name"]
            }),
        },
        Tool {
            action: "compat.list_prefixes",
            description: "Enumerate every Wine prefix the user has under ~/.jarvis/wine/, with \
                          metadata (initialised, created_at, last_used_at).",
            schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            action: "compat.run_proton",
            description: "Run a Windows .exe under Proton-GE in a named prefix (lives at \
                          ~/.jarvis/proton-data/<prefix>/). Use this for games and apps that \
                          need DXVK/VKD3D. Returns a clear 'proton not installed' error if \
                          the user hasn't installed Proton-GE yet — chain compat.install_proton \
                          first when that happens.",
            schema: json!({
                "type": "object",
                "properties": {
                    "prefix": {
                        "type": "string",
                        "description": "Prefix name (same naming rules as run_exe_in)"
                    },
                    "path": { "type": "string", "description": "Absolute path to the .exe" },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["prefix", "path"]
            }),
        },
        Tool {
            action: "compat.install_proton",
            description: "Download + extract Proton-GE (~310 MB) to ~/.jarvis/proton-ge/. \
                          Idempotent — returns immediately if it's already installed. Streams \
                          progress through a notification toast. Call this when compat.run_proton \
                          fails with 'proton not installed'.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            action: "compat.list_running",
            description: "Snapshot of every Wine / Proton child the daemon is currently tracking. \
                          Returns [{ pid, prefix, engine, exe, started_at }]. Useful before \
                          terminating something — picks the right pid by name.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            action: "compat.terminate",
            description: "SIGTERM a tracked Wine / Proton process by pid. Use compat.list_running \
                          first to find the pid. Refuses pids the daemon isn't tracking (avoids \
                          accidentally killing unrelated processes).",
            schema: json!({
                "type": "object",
                "properties": {
                    "pid": {
                        "type": "integer",
                        "description": "PID returned by a previous compat.run_exe / run_proton call"
                    }
                },
                "required": ["pid"]
            }),
        },
        // ── updater ─────────────────────────────────────────────────────
        Tool {
            action: "updater.check",
            description: "Check whether the Lilith model is installed and whether a Jarvis OS \
                          upgrade is staged. Returns model_present, os_update_available, \
                          os_version.",
            schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            action: "updater.apply_os",
            description: "Apply the pending bootc OS upgrade. The user must explicitly confirm \
                          this — Lilith should ask before calling it. A reboot is required to \
                          finish; this action only stages.",
            schema: json!({ "type": "object", "properties": {} }),
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
        Tool {
            action: "memory.search",
            description: "Search past conversation turns by substring (case-insensitive). Use \
                          when the user asks 'o que falamos sobre X' / 'lembra quando eu disse \
                          Y' / 'achei já ter perguntado isso'. Returns matches newest-first with \
                          timestamp + the original user/reply text — you can then quote back \
                          to the user. Different from memory.recall, which fetches a single \
                          named fact.",
            schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Substring to look for in user prompts + Lilith replies." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50, "description": "Max matches to return (default 5)." }
                },
                "required": ["query"]
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

/// Selector schema for the foreign-toplevel-backed window actions (ADR
/// 0025). `target` is a string — "active"/"focused" for the focused
/// window, an app name (matched against app_id), or a title substring.
/// Optional: omitting it defaults to the focused window.
fn window_target_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "target": {
                "type": "string",
                "description": "\"active\" for the focused window, an app name (e.g. \"firefox\"), or a title substring."
            }
        }
    })
}

/// Human-readable capability listing, built at runtime from
/// `all_tools()`. Used by the help intent (`/help`, "o que você sabe
/// fazer?") so the reply tracks the actual tool catalog instead of
/// drifting from a hardcoded string.
///
/// Groups by namespace prefix (`app.*`, `file.*`, …). Known
/// namespaces get a pt-BR label; unknown ones fall back to the
/// namespace identifier itself so new groups still show up.
pub fn help_text() -> String {
    use std::collections::BTreeMap;

    // Friendly labels per namespace. Order here is the display order
    // — picked to match the "set up my desktop" arc the empty-state
    // suggestions follow.
    const LABELS: &[(&str, &str)] = &[
        ("app", "aplicativos"),
        ("browser", "navegador"),
        ("screenshot", "screenshots"),
        ("clipboard", "área de transferência"),
        ("audio", "áudio"),
        ("window", "janelas"),
        ("workspace", "workspaces"),
        ("file", "arquivos"),
        ("system", "sistema"),
        ("compat", "Windows (Wine/Proton)"),
        ("updater", "atualizações"),
        ("memory", "memória"),
    ];

    let mut groups: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for tool in all_tools() {
        let ns = tool.action.split('.').next().unwrap_or(tool.action);
        groups.entry(ns).or_default().push(tool.action.to_string());
    }

    let mut body = String::new();
    let mut emitted: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (ns, label) in LABELS {
        if let Some(actions) = groups.get(ns) {
            // Two-space padding-as-separator keeps the layout legible
            // even when the namespace label has accented characters
            // (width still counts code points).
            body.push_str(&format!("• {label:<22}— {}\n", actions.join(", ")));
            emitted.insert(ns);
        }
    }
    // Any namespace we didn't list in LABELS shows up too, with its
    // raw identifier as the label. Keeps the help honest when new
    // tool groups land before this table catches up.
    for (ns, actions) in &groups {
        if !emitted.contains(ns) {
            body.push_str(&format!("• {ns:<22}— {}\n", actions.join(", ")));
        }
    }

    format!(
        "Posso fazer isso aqui pra você:\n\n{body}\nPergunte em português ou inglês — \
         eu encadeio várias ações quando faz sentido (\"tira um print e abre no editor\")."
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_text_lists_every_namespace_in_the_catalog() {
        let h = help_text();
        // Every namespace from all_tools() must appear at least once,
        // either through the labeled section or the fallback raw
        // identifier branch.
        let mut namespaces: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for tool in all_tools() {
            namespaces.insert(tool.action.split('.').next().unwrap_or(tool.action));
        }
        for ns in &namespaces {
            let label = label_for_test(ns);
            assert!(
                h.contains(ns) || h.contains(label.as_str()),
                "namespace {ns} not in help_text:\n{h}"
            );
        }
    }

    /// Mirrors LABELS in help_text — used in the assertion above.
    /// Kept in sync manually for now; a future commit could expose
    /// LABELS publicly if drift becomes a real problem.
    fn label_for_test(ns: &str) -> String {
        match ns {
            "app" => "aplicativos".to_string(),
            "browser" => "navegador".to_string(),
            "screenshot" => "screenshots".to_string(),
            "clipboard" => "área de transferência".to_string(),
            "audio" => "áudio".to_string(),
            "window" => "janelas".to_string(),
            "workspace" => "workspaces".to_string(),
            "file" => "arquivos".to_string(),
            "system" => "sistema".to_string(),
            "compat" => "Windows (Wine/Proton)".to_string(),
            "updater" => "atualizações".to_string(),
            "memory" => "memória".to_string(),
            other => other.to_string(),
        }
    }

    #[test]
    fn help_text_includes_chain_hint() {
        let h = help_text();
        assert!(h.contains("encadeio"));
    }
}
