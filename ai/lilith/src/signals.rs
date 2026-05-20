//! `SignalSink` — abstraction over the DBus signal emissions Lilith
//! makes during `process()`. Lets tests drive the chain loop without
//! a live `SignalContext` (which needs a real connection).
//!
//! Two implementations:
//!
//! - `DbusSignalSink` (production): wraps an owned `SignalContext` and
//!   forwards `partial_reply` / `chain_step` to the
//!   `com.jarvis.Lilith` signals declared on `LilithService`.
//!
//! - `NoopSink` (#[cfg(test)]): swallows every emission. Plus a
//!   `RecordingSink` that captures `(step, payload)` tuples so tests
//!   can assert "the chain emitted these steps in this order".
//!
//! Production wires this up in `command()`: the `#[zbus(signal_context)]`
//! parameter is wrapped in a `DbusSignalSink` and passed into
//! `process()` as `Arc<dyn SignalSink>`. The forwarder tasks spawned
//! per chain step `Arc::clone` the sink into a 'static-friendly handle.

use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait SignalSink: Send + Sync {
    /// One token batch from Ollama's stream. `step` is the chain-step
    /// index this chunk belongs to so the UI can colour transitions
    /// between tools and the final free-text wrap-up.
    async fn partial_reply(&self, step: u32, chunk: &str);

    /// Fires right before a tool dispatches in the chain loop. UI
    /// renders "Lilith → screenshot.capture…" while the (possibly
    /// slow) Action Bus call runs.
    async fn chain_step(&self, step: u32, action: &str);
}

/// No-op convenience — useful when the caller doesn't have a real
/// signal context handy (early init, batch use, tests).
pub struct NoopSink;

#[async_trait]
impl SignalSink for NoopSink {
    async fn partial_reply(&self, _step: u32, _chunk: &str) {}
    async fn chain_step(&self, _step: u32, _action: &str) {}
}

/// Convenience constructor — Lilith uses this when standing up the
/// service in case it needs a placeholder while wiring the real
/// `DbusSignalSink`.
pub fn noop_sink() -> Arc<dyn SignalSink> {
    Arc::new(NoopSink)
}
