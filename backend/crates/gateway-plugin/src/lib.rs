//! # gateway-plugin — Plugin framework for Sentinel Gateway
//!
//! Plugins hook into the request/response lifecycle of LLM and proxy calls.
//! They can inspect, modify, block, or side-effect on every stage.
//!
//! ## Lifecycle Hooks
//!
//! ```text
//! Request arrives
//!     │
//!     ▼
//!  before_request()     ← plugins may redact, transform, or block
//!     │
//!     ▼
//!  Forward to backend
//!     │
//!     ▼
//!  after_response()     ← plugins may redact, augment, or record
//!     │
//!     ▼
//!  Return to client
//! ```
//!
//! ## Plugin Kinds
//!
//! - **Input** (request transformers): PII redaction, prompt injection, templating
//! - **Output** (response transformers): content filters, augmentation
//! - **Guardrail**: pass/fail decision, optionally block
//! - **Observer**: side-effects only (logs, metrics, exports), no mutation
//! - **Auth**: additional auth methods (e.g., custom header validators)
//!
//! Plugins run in **ordered pipelines** per kind. The first plugin to return
//! `PluginDecision::Block` short-circuits the chain. The `Observer` kind
//! never blocks.

pub mod context;
pub mod decision;
pub mod error;
pub mod plugin;
pub mod registry;

#[cfg(test)]
mod tests;

pub use context::{PluginContext, RequestPhase};
pub use decision::{PluginDecision, PluginOutcome};
pub use error::PluginError;
pub use plugin::{Plugin, PluginKind, PluginMetadata};
pub use registry::PluginRegistry;
