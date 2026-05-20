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

/// Returns true when `text` is asking what Lilith can do. Handled
/// outside the regular rule path because the response is a hardcoded
/// capability listing, not an Action Bus dispatch.
///
/// Match-anywhere on a normalized prompt — phrasings like
/// "diga o que você sabe fazer" should hit too. Kept loose
/// because the cost of a false positive ("ajuda dela com isso")
/// is just a more verbose reply.
pub fn is_help_query(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    if t == "/help" || t == "/ajuda" {
        return true;
    }
    const NEEDLES: &[&str] = &[
        "o que voce sabe fazer",
        "o que você sabe fazer",
        "o que voce pode fazer",
        "o que você pode fazer",
        "what can you do",
        "what do you know",
        "como posso te usar",
        "como te usar",
        "lista de comandos",
        "list of commands",
    ];
    NEEDLES.iter().any(|n| t.contains(n))
        // Standalone "ajuda" / "help" hits too, but we don't want to
        // match "preciso de ajuda para X" — that's the user asking for
        // help on a task, not for our capabilities. Require it to be
        // the whole input.
        || matches!(t.as_str(), "ajuda" | "help")
}

/// If the user's text is a math / unit-conversion question,
/// returns the bare expression to feed to `numbat`. None means
/// "not a calc query" and the regular rule/LLM path runs.
///
/// Detected forms (case-insensitive, accent-tolerant):
///   - "quanto é <expr>"   / "quanto eh <expr>"
///   - "quanto vale <expr>"
///   - "calcular <expr>"   / "calcula <expr>"   / "calc <expr>"
///   - "converter <expr>"  / "convert <expr>"
///   - "what is <expr>"    / "what's <expr>"
///
/// We strip a leading "?" / "." / trailing punctuation from the
/// captured expression so "quanto é 2 + 2?" doesn't reach numbat
/// as "2 + 2?" (which numbat would refuse).
pub fn extract_calc_expression(text: &str) -> Option<String> {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"(?i)^(?:quanto\s+(?:é|eh|vale)|calcul(?:ar|a)|calc|convert(?:er)?|what(?:\s+is|'s))\s+(?P<expr>.+?)[?.!]?$",
        )
        .unwrap()
    });
    let t = text.trim();
    let caps = RE.captures(t)?;
    let expr = caps.name("expr")?.as_str().trim().to_string();
    if expr.is_empty() {
        return None;
    }
    Some(expr)
}

struct Rule {
    pattern: Regex,
    build: fn(&regex::Captures) -> ToolCall,
}

/// Resolve `$HOME` once at first rule-match. The lazy `OnceCell`
/// avoids re-calling `dirs::home_dir()` on every parse.
fn home() -> String {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".into())
}

/// Map an XDG folder name token (in either pt-BR or English) to the
/// absolute path under $HOME. We hardcode the English XDG defaults —
/// localised folder names ("Documentos" instead of "Documents") only
/// kick in when `xdg-user-dirs` is configured per-user, which Jarvis
/// OS doesn't ship today. Phase 24+ candidate: read
/// `$XDG_CONFIG_HOME/user-dirs.dirs` and translate.
fn xdg_folder_path(name: &str) -> String {
    let sub = match name.to_lowercase().as_str() {
        "downloads" => "Downloads",
        "documentos" | "documents" => "Documents",
        "imagens" | "images" | "pictures" => "Pictures",
        "música" | "musica" | "music" => "Music",
        "vídeos" | "videos" => "Videos",
        _ => return home(),
    };
    format!("{}/{}", home(), sub)
}

static RULES: Lazy<Vec<Rule>> = Lazy::new(|| {
    vec![
        // ── File navigation — opens Dolphin (via xdg-open) at the
        //    matching XDG folder. The MIME default
        //    (inode/directory → org.kde.dolphin.desktop) handles the
        //    routing; we just hand xdg-open the path. Matched before
        //    the generic `abrir <app>` rule so "abrir downloads"
        //    doesn't try to launch a `downloads` executable.
        Rule {
            pattern: Regex::new(
                r"^(?:abr(?:ir|a)|open)\s+(?P<folder>downloads|documentos|documents|imagens|images|pictures|m[uú]sica|music|v[ií]deos|videos)$",
            )
            .unwrap(),
            build: |c| ToolCall {
                action: "app.open".into(),
                params: json!({ "app": xdg_folder_path(&c["folder"]) }),
            },
        },
        Rule {
            pattern: Regex::new(
                r"^(?:abr(?:ir|a)|open)\s+(?:arquivos|files|home|minha\s+pasta|pasta\s+pessoal)$",
            )
            .unwrap(),
            build: |_| ToolCall {
                action: "app.open".into(),
                params: json!({ "app": home() }),
            },
        },

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
        // ── browser ────────────────────────────────────────────────────
        // "open https://…" / "abrir https://…"
        Rule {
            pattern: Regex::new(
                r"^(?:open|abr(?:ir|a))\s+(?P<url>https?://\S+|mailto:\S+)$",
            )
            .unwrap(),
            build: |c| ToolCall {
                action: "browser.open".into(),
                params: json!({ "url": &c["url"] }),
            },
        },
        // ── clipboard ──────────────────────────────────────────────────
        // "copy: <text>" / "copia: <text>" / "copiar: <text>"
        Rule {
            pattern: Regex::new(r"^(?:copy|copi(?:ar|a|e))[:\s]+(?P<text>.+)$").unwrap(),
            build: |c| ToolCall {
                action: "clipboard.set".into(),
                params: json!({ "text": c["text"].trim() }),
            },
        },
        // "paste" / "cola" / "colar" — bare verb, no argument
        Rule {
            pattern: Regex::new(r"^(?:paste|col(?:ar|a|e))$").unwrap(),
            build: |_| ToolCall {
                action: "clipboard.get".into(),
                params: json!({}),
            },
        },
        // ── screenshot ─────────────────────────────────────────────────
        // "screenshot" / "print" / "captura tela" / "tira print"
        Rule {
            pattern: Regex::new(
                r"^(?:screenshot|print(?:\s+screen)?|captura(?:r)?\s+(?:a\s+)?tela|tir(?:ar|a|e)\s+print)$",
            )
            .unwrap(),
            build: |_| ToolCall {
                action: "screenshot.capture".into(),
                params: json!({ "mode": "full" }),
            },
        },
        // "screenshot region" / "captura região" / "print área"
        Rule {
            pattern: Regex::new(
                r"^(?:screenshot|print|captura(?:r)?)\s+(?:region|área|area|região)$",
            )
            .unwrap(),
            build: |_| ToolCall {
                action: "screenshot.capture".into(),
                params: json!({ "mode": "region" }),
            },
        },
        // ── audio ──────────────────────────────────────────────────────
        // "volume 50" / "set volume 50" / "som 50"
        Rule {
            pattern: Regex::new(
                r"^(?:(?:set\s+)?volume|som)\s+(?P<pct>\d{1,3})$",
            )
            .unwrap(),
            build: |c| ToolCall {
                action: "audio.set_volume".into(),
                params: json!({ "percent": parse_id(&c["pct"]) }),
            },
        },
        // "louder" / "volume up" / "aumentar volume" / "subir som"
        Rule {
            pattern: Regex::new(
                r"^(?:louder|volume\s+up|aument(?:ar|a|e)\s+(?:o\s+)?(?:volume|som)|sub(?:ir|a|e)\s+(?:o\s+)?(?:volume|som))$",
            )
            .unwrap(),
            build: |_| ToolCall {
                action: "audio.adjust_volume".into(),
                params: json!({ "delta": 10 }),
            },
        },
        // "quieter" / "volume down" / "diminuir volume" / "abaixar som"
        Rule {
            pattern: Regex::new(
                r"^(?:quieter|volume\s+down|diminu(?:ir|a|e)\s+(?:o\s+)?(?:volume|som)|abaix(?:ar|a|e)\s+(?:o\s+)?(?:volume|som))$",
            )
            .unwrap(),
            build: |_| ToolCall {
                action: "audio.adjust_volume".into(),
                params: json!({ "delta": -10 }),
            },
        },
        // "mute" / "unmute" / "mutar" / "tira som"
        Rule {
            pattern: Regex::new(r"^(?:mute|mut(?:ar|a|e)|tir(?:ar|a|e)\s+(?:o\s+)?som)$").unwrap(),
            build: |_| ToolCall {
                action: "audio.toggle_mute".into(),
                params: json!({}),
            },
        },
        Rule {
            pattern: Regex::new(r"^(?:unmute|desmut(?:ar|a|e))$").unwrap(),
            build: |_| ToolCall {
                action: "audio.toggle_mute".into(),
                params: json!({ "set_state": false }),
            },
        },
        // ── memory ─────────────────────────────────────────────────────
        // "remember <key> = <value>"  or  "lembrar <key> = <value>"
        Rule {
            pattern: Regex::new(
                r"^(?:remember|lembr(?:ar|e|a))[:\s]+(?P<key>[^=]+?)\s*=\s*(?P<value>.+)$",
            )
            .unwrap(),
            build: |c| ToolCall {
                action: "memory.remember".into(),
                params: json!({ "key": c["key"].trim(), "value": c["value"].trim() }),
            },
        },
        // "recall <key>"  or  "lembrete <key>"
        Rule {
            pattern: Regex::new(r"^(?:recall|lembrete)\s+(?P<key>.+)$").unwrap(),
            build: |c| ToolCall {
                action: "memory.recall".into(),
                params: json!({ "key": c["key"].trim() }),
            },
        },
        // "forget <key>"  or  "esquecer/esqueça <key>"
        Rule {
            pattern: Regex::new(r"^(?:forget|esqu(?:ecer|eça|eca))\s+(?P<key>.+)$").unwrap(),
            build: |c| ToolCall {
                action: "memory.forget".into(),
                params: json!({ "key": c["key"].trim() }),
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
    fn parses_open_xdg_folder() {
        // The folder-specific rule must match BEFORE the generic
        // `abrir <app>` rule — otherwise "abrir downloads" would
        // try to launch a binary called `downloads`. This test pins
        // the order.
        let call = parse("abrir downloads").unwrap();
        assert_eq!(call.action, "app.open");
        let path = call.params["app"].as_str().unwrap_or("");
        assert!(path.ends_with("/Downloads"), "path was {path}");

        let call = parse("open documents").unwrap();
        assert!(call.params["app"].as_str().unwrap_or("").ends_with("/Documents"));

        let call = parse("abrir música").unwrap();
        assert!(call.params["app"].as_str().unwrap_or("").ends_with("/Music"));

        let call = parse("abrir vídeos").unwrap();
        assert!(call.params["app"].as_str().unwrap_or("").ends_with("/Videos"));
    }

    #[test]
    fn calc_extracts_expression() {
        assert_eq!(extract_calc_expression("quanto é 2 + 2"),
                   Some("2 + 2".into()));
        assert_eq!(extract_calc_expression("quanto é 3 metros + 4 pés"),
                   Some("3 metros + 4 pés".into()));
        // Trailing punctuation should be stripped.
        assert_eq!(extract_calc_expression("quanto é 5 * 5?"),
                   Some("5 * 5".into()));
        assert_eq!(extract_calc_expression("calcular 2^10"),
                   Some("2^10".into()));
        assert_eq!(extract_calc_expression("calc 1+1"),
                   Some("1+1".into()));
        assert_eq!(extract_calc_expression("converter 50 milhas para km"),
                   Some("50 milhas para km".into()));
        assert_eq!(extract_calc_expression("what is 1024 / 16"),
                   Some("1024 / 16".into()));
        // Empty body → None (the regex requires non-empty expr).
        assert_eq!(extract_calc_expression("calcular"), None);
        // Non-calc questions → None.
        assert_eq!(extract_calc_expression("abrir firefox"), None);
        assert_eq!(extract_calc_expression("o que você sabe fazer"), None);
    }

    #[test]
    fn parses_open_home() {
        let call = parse("abrir arquivos").unwrap();
        assert_eq!(call.action, "app.open");
        let path = call.params["app"].as_str().unwrap_or("");
        // Should be the bare $HOME, not a subfolder.
        assert!(!path.ends_with("/Downloads"));
        assert!(!path.is_empty());

        let call = parse("open files").unwrap();
        assert_eq!(call.action, "app.open");

        let call = parse("abrir minha pasta").unwrap();
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

    #[test]
    fn parses_memory_remember() {
        let call = parse("remember favorite editor = vscode").unwrap();
        assert_eq!(call.action, "memory.remember");
        assert_eq!(call.params["key"], "favorite editor");
        assert_eq!(call.params["value"], "vscode");

        let call = parse("lembrar idioma = pt-br").unwrap();
        assert_eq!(call.action, "memory.remember");
        assert_eq!(call.params["key"], "idioma");
        assert_eq!(call.params["value"], "pt-br");
    }

    #[test]
    fn parses_memory_recall() {
        let call = parse("recall favorite editor").unwrap();
        assert_eq!(call.action, "memory.recall");
        assert_eq!(call.params["key"], "favorite editor");
    }

    #[test]
    fn parses_memory_forget() {
        let call = parse("forget favorite editor").unwrap();
        assert_eq!(call.action, "memory.forget");
        assert_eq!(call.params["key"], "favorite editor");

        let call = parse("esquecer idioma").unwrap();
        assert_eq!(call.action, "memory.forget");
        assert_eq!(call.params["key"], "idioma");
    }

    #[test]
    fn parses_browser_open_https() {
        let call = parse("open https://example.com/foo?q=1").unwrap();
        assert_eq!(call.action, "browser.open");
        assert_eq!(call.params["url"], "https://example.com/foo?q=1");

        let call = parse("abrir https://github.com").unwrap();
        assert_eq!(call.action, "browser.open");
    }

    #[test]
    fn parses_browser_open_only_with_scheme() {
        // Bare hostnames must not match browser.open — they collide with
        // `app.open` ("abrir firefox"). The scheme is what tells us the user
        // wants a URL, not an app.
        let call = parse("open firefox").unwrap();
        assert_eq!(call.action, "app.open");
        assert_eq!(call.params["app"], "firefox");
    }

    #[test]
    fn parses_clipboard_set_english_and_pt() {
        let call = parse("copy: hello world").unwrap();
        assert_eq!(call.action, "clipboard.set");
        assert_eq!(call.params["text"], "hello world");

        let call = parse("copia: minha senha").unwrap();
        assert_eq!(call.action, "clipboard.set");
        assert_eq!(call.params["text"], "minha senha");
    }

    #[test]
    fn parses_clipboard_get_bare() {
        let call = parse("paste").unwrap();
        assert_eq!(call.action, "clipboard.get");

        let call = parse("cola").unwrap();
        assert_eq!(call.action, "clipboard.get");
    }

    #[test]
    fn parses_screenshot_full() {
        let call = parse("screenshot").unwrap();
        assert_eq!(call.action, "screenshot.capture");
        assert_eq!(call.params["mode"], "full");

        let call = parse("tirar print").unwrap();
        assert_eq!(call.action, "screenshot.capture");
        assert_eq!(call.params["mode"], "full");
    }

    #[test]
    fn parses_screenshot_region() {
        let call = parse("screenshot region").unwrap();
        assert_eq!(call.action, "screenshot.capture");
        assert_eq!(call.params["mode"], "region");

        let call = parse("captura região").unwrap();
        assert_eq!(call.action, "screenshot.capture");
        assert_eq!(call.params["mode"], "region");
    }

    #[test]
    fn parses_audio_set_volume() {
        let call = parse("volume 50").unwrap();
        assert_eq!(call.action, "audio.set_volume");
        assert_eq!(call.params["percent"], 50);

        let call = parse("som 75").unwrap();
        assert_eq!(call.action, "audio.set_volume");
        assert_eq!(call.params["percent"], 75);
    }

    #[test]
    fn parses_audio_adjust_up_and_down() {
        let up = parse("aumentar volume").unwrap();
        assert_eq!(up.action, "audio.adjust_volume");
        assert_eq!(up.params["delta"], 10);

        let down = parse("diminuir som").unwrap();
        assert_eq!(down.action, "audio.adjust_volume");
        assert_eq!(down.params["delta"], -10);

        let louder = parse("louder").unwrap();
        assert_eq!(louder.params["delta"], 10);
    }

    #[test]
    fn parses_audio_mute_toggle_and_unmute() {
        let mute = parse("mute").unwrap();
        assert_eq!(mute.action, "audio.toggle_mute");
        // toggle path — no set_state in params
        assert!(mute.params.get("set_state").is_none());

        let unmute = parse("unmute").unwrap();
        assert_eq!(unmute.action, "audio.toggle_mute");
        assert_eq!(unmute.params["set_state"], false);

        let mutar = parse("tirar som").unwrap();
        assert_eq!(mutar.action, "audio.toggle_mute");
    }
}
