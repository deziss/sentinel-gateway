use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::context::PluginContext;
use crate::decision::PluginDecision;
use crate::error::PluginError;

/// Plugin kind determines which pipeline stage it runs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// Transforms / validates the inbound request.
    Input,
    /// Transforms / filters the outbound response.
    Output,
    /// Pass/fail guard on inbound content (PII, prompt injection, etc.).
    Guardrail,
    /// Side-effect only (logs, metrics, export). Runs after every request.
    Observer,
    /// Custom auth method (e.g., verify a third-party HMAC header).
    Auth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub kind: PluginKind,
    /// Lower priority runs first. Ties broken by name.
    pub priority: i32,
    /// Whether the plugin is enabled at runtime.
    pub enabled: bool,
    pub description: Option<String>,
}

/// Core trait every plugin implements.
///
/// Plugins are `Send + Sync` so they can live in an `Arc<dyn Plugin>` and
/// be called from any task. Implementations should be cheap to clone
/// (typically `Arc<Config>` internally).
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Plugin metadata (name, kind, priority).
    fn metadata(&self) -> &PluginMetadata;

    /// Convenience: the plugin name.
    fn name(&self) -> &str {
        &self.metadata().name
    }

    /// Convenience: the plugin kind.
    fn kind(&self) -> PluginKind {
        self.metadata().kind
    }

    /// Called before the request is forwarded to the backend.
    /// Default: pass through unchanged.
    async fn before_request(&self, _ctx: &mut PluginContext) -> Result<PluginDecision, PluginError> {
        Ok(PluginDecision::Continue)
    }

    /// Called after the backend response is received.
    /// Default: pass through unchanged.
    async fn after_response(&self, _ctx: &mut PluginContext) -> Result<PluginDecision, PluginError> {
        Ok(PluginDecision::Continue)
    }

    /// Called if the backend request itself errored out.
    /// Default: no-op.
    async fn on_error(&self, _ctx: &mut PluginContext, _error: &str) -> Result<(), PluginError> {
        Ok(())
    }
}
