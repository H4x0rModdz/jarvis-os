use crate::tools::ToolCall;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::json;

/// Rule-based intent matcher.
///
/// Deterministic, runs before any LLM call. Handles common phrases in
/// Portuguese and English. Returns `None` when no rule matches — the caller
/// then falls back to Ollama (if reachable) or returns an "unknown intent"
/// response.
pub fn parse(text: &str) -> Option<ToolCall> {
    let t = text.trim().to_lowercase();

    for rule in RULES.iter() {
        if let Some(caps) = rule.pattern.captures(&t) {
            return Some((rule.build)(&caps));
        }
    }
    None
}

struct Rule {
    pattern: Regex,
    build: fn(&regex::Captures) -> ToolCall,
}

static RULES: Lazy<Vec<Rule>> = Lazy::new(|| {
    vec![
        // ── app.open ───────────────────────────────────────────────────
        Rule {
            pattern: Regex::new(r"^(?:open|launch|abr(?:ir|a)|inicia[r]?)\s+(?P<app>[\w\-.]+)$")
                .unwrap(),
            build: |c| ToolCall {
                action: "app.open".into(),
                params: json!({ "app": &c["app"] }),
            },
        },
        // ── app.close ──────────────────────────────────────────────────
        Rule {
            pattern: Regex::new(
                r"^(?:close|quit|fech(?:ar|e)|encerr(?:ar|e))\s+(?P<app>[\w\-.]+)$",
            )
            .unwrap(),
            build: |c| ToolCall {
                action: "app.close".into(),
                params: json!({ "app": &c["app"] }),
            },
        },
        // ── window control (numeric id) ────────────────────────────────
        Rule {
            pattern: Regex::new(r"^(?:focus|focar)\s+window\s+(?P<id>\d+)$").unwrap(),
            build: |c| ToolCall {
                action: "window.focus".into(),
                params: json!({ "window_id": parse_id(&c["id"]) }),
            },
        },
        Rule {
            pattern: Regex::new(r"^(?:minimize|minimizar?)\s+window\s+(?P<id>\d+)$").unwrap(),
            build: |c| ToolCall {
                action: "window.minimize".into(),
                params: json!({ "window_id": parse_id(&c["id"]) }),
            },
        },
        Rule {
            pattern: Regex::new(r"^(?:maximize|maximizar?)\s+window\s+(?P<id>\d+)$").unwrap(),
            build: |c| ToolCall {
                action: "window.maximize".into(),
                params: json!({ "window_id": parse_id(&c["id"]) }),
            },
        },
        Rule {
            pattern: Regex::new(r"^(?:close|fechar?)\s+window\s+(?P<id>\d+)$").unwrap(),
            build: |c| ToolCall {
                action: "window.close".into(),
                params: json!({ "window_id": parse_id(&c["id"]) }),
            },
        },
        Rule {
            pattern: Regex::new(
                r"^(?:snap\s+left|encaixar\s+(?:a\s+)?esquerda)\s+window\s+(?P<id>\d+)$",
            )
            .unwrap(),
            build: |c| ToolCall {
                action: "window.snap_left".into(),
                params: json!({ "window_id": parse_id(&c["id"]) }),
            },
        },
        Rule {
            pattern: Regex::new(
                r"^(?:snap\s+right|encaixar\s+(?:a\s+)?direita)\s+window\s+(?P<id>\d+)$",
            )
            .unwrap(),
            build: |c| ToolCall {
                action: "window.snap_right".into(),
                params: json!({ "window_id": parse_id(&c["id"]) }),
            },
        },
        // ── notification ───────────────────────────────────────────────
        Rule {
            pattern: Regex::new(r"^(?:notify|notificar?)[:\s]+(?P<body>.+)$").unwrap(),
            build: |c| ToolCall {
                action: "system.notify".into(),
                params: json!({ "title": "Jarvis", "body": &c["body"] }),
            },
        },
        // ── workspace ──────────────────────────────────────────────────
        Rule {
            pattern: Regex::new(
                r"^(?:switch\s+to\s+workspace|mudar\s+para\s+workspace|ir\s+para\s+workspace)\s+(?P<idx>\d+)$",
            )
            .unwrap(),
            build: |c| ToolCall {
                action: "workspace.switch".into(),
                params: json!({ "index": parse_id(&c["idx"]) }),
            },
        },
        Rule {
            pattern: Regex::new(r"^(?:create\s+workspace|criar\s+workspace)$").unwrap(),
            build: |_| ToolCall {
                action: "workspace.create".into(),
                params: json!({}),
            },
        },
    ]
});

fn parse_id(s: &str) -> u64 {
    s.parse().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_app_english() {
        let call = parse("open vscode").unwrap();
        assert_eq!(call.action, "app.open");
        assert_eq!(call.params["app"], "vscode");
    }

    #[test]
    fn parses_open_app_portuguese() {
        let call = parse("abrir firefox").unwrap();
        assert_eq!(call.action, "app.open");
        assert_eq!(call.params["app"], "firefox");

        let call = parse("abra firefox").unwrap();
        assert_eq!(call.action, "app.open");
    }

    #[test]
    fn parses_close_app() {
        let call = parse("fechar slack").unwrap();
        assert_eq!(call.action, "app.close");
        assert_eq!(call.params["app"], "slack");
    }

    #[test]
    fn parses_window_minimize() {
        let call = parse("minimize window 42").unwrap();
        assert_eq!(call.action, "window.minimize");
        assert_eq!(call.params["window_id"], 42);
    }

    #[test]
    fn parses_snap_left_portuguese() {
        let call = parse("encaixar esquerda window 7").unwrap();
        assert_eq!(call.action, "window.snap_left");
        assert_eq!(call.params["window_id"], 7);
    }

    #[test]
    fn parses_notification() {
        let call = parse("notify: meeting in 5 minutes").unwrap();
        assert_eq!(call.action, "system.notify");
        assert_eq!(call.params["body"], "meeting in 5 minutes");
    }

    #[test]
    fn parses_workspace_switch() {
        let call = parse("switch to workspace 3").unwrap();
        assert_eq!(call.action, "workspace.switch");
        assert_eq!(call.params["index"], 3);
    }

    #[test]
    fn unknown_intent_returns_none() {
        assert!(parse("what is the meaning of life").is_none());
        assert!(parse("hello there").is_none());
        assert!(parse("").is_none());
    }

    #[test]
    fn case_insensitive() {
        assert!(parse("OPEN VSCode").is_some());
        assert!(parse("Abrir Firefox").is_some());
    }
}
