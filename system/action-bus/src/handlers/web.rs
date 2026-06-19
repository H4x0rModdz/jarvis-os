use crate::error::BusError;
use scraper::{Html, Selector};
use serde_json::{json, Value};

const DEFAULT_SEARCH_URL: &str = "https://lite.duckduckgo.com/lite/";
// DDG (and most engines) reject an empty/curl UA — present as a browser.
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

/// Search the web. Returns up to `limit` results `[{title, url, snippet}]`.
///
/// Backend defaults to DuckDuckGo Lite (no API key); override the endpoint
/// with `JARVIS_WEB_SEARCH_URL` (e.g. a SearXNG instance). The HTML parse is
/// best-effort and the most likely thing to need a tweak if DDG changes its
/// markup — keep it isolated here.
pub async fn search(params: Value) -> Result<Value, BusError> {
    let query = params["query"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'query'".into(),
        })?;
    let limit = params["limit"].as_u64().unwrap_or(5).clamp(1, 10) as usize;

    let base = std::env::var("JARVIS_WEB_SEARCH_URL").unwrap_or_else(|_| DEFAULT_SEARCH_URL.into());
    let html = http_client()?
        .get(&base)
        .query(&[("q", query)])
        .send()
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("web search: {e}"),
        })?
        .text()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("read search response: {e}"),
        })?;

    let results = parse_ddg_lite(&html, limit);
    Ok(json!({ "query": query, "results": results }))
}

/// Fetch a web page and return its readable text (scripts/markup stripped,
/// truncated). For "read this and summarise" chains.
pub async fn fetch(params: Value) -> Result<Value, BusError> {
    let url = params["url"]
        .as_str()
        .ok_or_else(|| BusError::InvalidParams {
            message: "missing required param 'url'".into(),
        })?;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(BusError::InvalidParams {
            message: "url must be http(s)".into(),
        });
    }
    let max = params["max_chars"]
        .as_u64()
        .unwrap_or(4000)
        .clamp(200, 20000) as usize;

    let html = http_client()?
        .get(url)
        .send()
        .await
        .map_err(|e| BusError::Unavailable {
            service: format!("fetch {url}: {e}"),
        })?
        .text()
        .await
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("read page: {e}"),
        })?;

    let mut text = html_to_text(&html);
    if text.chars().count() > max {
        text = text.chars().take(max).collect();
    }
    Ok(json!({ "url": url, "text": text }))
}

/// Shared HTTP client (browser UA + sane timeout). Used by web.* and the
/// wallpaper handler's downloads.
pub(crate) fn http_client() -> Result<reqwest::Client, BusError> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| BusError::ExecutionFailed {
            message: format!("http client: {e}"),
        })
}

/// Parse DuckDuckGo Lite results. Each result is `a.result-link` (title +
/// href) paired by position with a `.result-snippet`.
fn parse_ddg_lite(html: &str, limit: usize) -> Vec<Value> {
    let doc = Html::parse_document(html);
    let link_sel = Selector::parse("a.result-link").unwrap();
    let snip_sel = Selector::parse(".result-snippet").unwrap();
    let snippets: Vec<String> = doc
        .select(&snip_sel)
        .map(|s| collapse_ws(&s.text().collect::<String>()))
        .collect();

    let mut out = Vec::new();
    for (i, a) in doc.select(&link_sel).enumerate() {
        if out.len() >= limit {
            break;
        }
        let url = clean_ddg_href(a.value().attr("href").unwrap_or(""));
        if url.is_empty() {
            continue;
        }
        out.push(json!({
            "title": collapse_ws(&a.text().collect::<String>()),
            "url": url,
            "snippet": snippets.get(i).cloned().unwrap_or_default(),
        }));
    }
    out
}

/// Pull readable text out of a page: the content elements, scripts/styles/nav
/// dropped by only selecting prose tags.
fn html_to_text(html: &str) -> String {
    let doc = Html::parse_document(html);
    let sel = Selector::parse("p, h1, h2, h3, h4, li, article, blockquote").unwrap();
    let mut parts = Vec::new();
    for el in doc.select(&sel) {
        let t = collapse_ws(&el.text().collect::<String>());
        if !t.is_empty() {
            parts.push(t);
        }
    }
    parts.join("\n")
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// DDG Lite wraps result URLs in a `/l/?uddg=<percent-encoded>` redirect.
/// Unwrap it to the real URL; pass through normal hrefs (prefixing `//`).
fn clean_ddg_href(href: &str) -> String {
    if let Some(idx) = href.find("uddg=") {
        let enc = &href[idx + 5..];
        let enc = enc.split('&').next().unwrap_or(enc);
        return percent_decode(enc);
    }
    if let Some(rest) = href.strip_prefix("//") {
        return format!("https://{rest}");
    }
    href.to_string()
}

/// Minimal percent-decoder (avoids pulling a urlencoding crate for one use).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
