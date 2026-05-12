//! `sdk-hello` — the canonical example Jarvis SDK app.
//!
//! Hosts `com.jarvis.app.sdk_hello` on the session bus. Implements
//! two demo actions:
//!
//!   - `sdk_hello.echo { message }` → `{ result: { echo: <message> } }`
//!   - `sdk_hello.add { a, b }`     → `{ result: { sum: a+b } }`
//!
//! Lilith picks these up automatically: `ListActions()` includes them
//! because the Action Bus loaded the manifest at startup, and Ollama
//! gets the action descriptions via the tools payload. Try
//! "diga oi ao sdk_hello" or "soma 2 e 3" in the bar.

use serde_json::{json, Value};
use zbus::{connection, interface};

struct HelloService;

#[interface(name = "com.jarvis.app.sdk_hello")]
impl HelloService {
    /// Action Bus contract: a single `Dispatch(action, params_json)`
    /// that returns a JSON string envelope. The bus parses the
    /// envelope into either `result` or `error`.
    async fn dispatch(&self, action: &str, params_json: &str) -> String {
        let params: Value = serde_json::from_str(params_json).unwrap_or(Value::Null);
        let outcome = match action {
            "sdk_hello.echo" => echo(&params),
            "sdk_hello.add" => add(&params),
            other => Err(format!("unknown action: {other}")),
        };

        match outcome {
            Ok(value) => json!({ "result": value }).to_string(),
            Err(message) => json!({
                "error": {
                    "code": "execution_failed",
                    "message": message,
                }
            })
            .to_string(),
        }
    }
}

fn echo(params: &Value) -> Result<Value, String> {
    let message = params
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing required param 'message'".to_string())?;
    Ok(json!({ "echo": message }))
}

fn add(params: &Value) -> Result<Value, String> {
    let a = params
        .get("a")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "missing or non-numeric param 'a'".to_string())?;
    let b = params
        .get("b")
        .and_then(|v| v.as_f64())
        .ok_or_else(|| "missing or non-numeric param 'b'".to_string())?;
    Ok(json!({ "sum": a + b }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sdk_hello=info".parse()?),
        )
        .init();

    tracing::info!("Starting sdk-hello (Jarvis SDK example)");

    let _conn = connection::Builder::session()?
        .name("com.jarvis.app.sdk_hello")?
        .serve_at("/com/jarvis/app/sdk_hello", HelloService)?
        .build()
        .await?;

    tracing::info!("Ready on com.jarvis.app.sdk_hello");

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_returns_the_message() {
        let r = echo(&json!({ "message": "hi" })).unwrap();
        assert_eq!(r["echo"], "hi");
    }

    #[test]
    fn echo_requires_message() {
        assert!(echo(&json!({})).is_err());
    }

    #[test]
    fn add_sums_two_numbers() {
        let r = add(&json!({ "a": 2.0, "b": 3.0 })).unwrap();
        assert_eq!(r["sum"], 5.0);
    }

    #[test]
    fn add_rejects_strings() {
        assert!(add(&json!({ "a": "x", "b": "y" })).is_err());
    }
}
