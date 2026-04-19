use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// Request lifecycle phase — determines which plugin hook is invoked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestPhase {
    BeforeRequest,
    AfterResponse,
    OnError,
}

/// Context passed to every plugin invocation.
///
/// Wraps all the state a plugin might need: tenant, user, route, body,
/// and mutable metadata for cross-plugin communication.
#[derive(Debug, Clone)]
pub struct PluginContext {
    pub phase: RequestPhase,
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub api_key_id: Option<Uuid>,
    pub virtual_key_id: Option<Uuid>,
    pub backend_id: Option<Uuid>,
    pub model: Option<String>,
    pub path: String,
    /// Request body (JSON). Plugins may mutate this in `before_request`.
    pub request: Value,
    /// Response body (JSON). Populated in `after_response` phase.
    pub response: Option<Value>,
    pub status_code: Option<u16>,
    /// Free-form metadata bag for inter-plugin state sharing
    /// (e.g., PII plugin writes `"pii_detected": true` for logging plugins).
    pub metadata: HashMap<String, Value>,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
}

impl PluginContext {
    /// Get a metadata value set by a previous plugin.
    pub fn get_metadata(&self, key: &str) -> Option<&Value> {
        self.metadata.get(key)
    }

    /// Set metadata for downstream plugins to consume.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: Value) {
        self.metadata.insert(key.into(), value);
    }

    /// Convenience: mark that PII was detected in this request.
    pub fn mark_pii_detected(&mut self) {
        self.set_metadata("pii_detected", Value::Bool(true));
    }

    /// Convenience: check if a previous plugin flagged this as cached.
    pub fn is_cached(&self) -> bool {
        self.metadata.get("cached")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}
