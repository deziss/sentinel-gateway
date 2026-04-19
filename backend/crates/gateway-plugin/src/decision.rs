use serde::{Deserialize, Serialize};

/// Decision returned by a plugin after executing its hook.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PluginDecision {
    /// Allow the request to continue down the pipeline.
    Continue,
    /// Request was modified. Pipeline continues with the new state.
    /// The plugin has already written its changes into `PluginContext`.
    Modified,
    /// Block the request with an error message. Pipeline short-circuits.
    /// The gateway returns a 4xx/5xx to the caller based on `status_code`.
    Block {
        status_code: u16,
        message: String,
    },
    /// Return a synthetic response immediately without hitting the backend.
    /// Used by cache plugins to serve cache hits.
    Respond {
        status_code: u16,
        body: serde_json::Value,
    },
}

impl PluginDecision {
    pub fn is_terminal(&self) -> bool {
        matches!(self, PluginDecision::Block { .. } | PluginDecision::Respond { .. })
    }
}

/// Final outcome of a plugin pipeline execution.
#[derive(Debug, Clone)]
pub struct PluginOutcome {
    pub decision: PluginDecision,
    /// Plugin that produced the terminal decision (if any).
    pub terminated_by: Option<String>,
    /// Per-plugin execution metadata (for debugging + observability).
    pub executions: Vec<PluginExecution>,
}

#[derive(Debug, Clone)]
pub struct PluginExecution {
    pub plugin_name: String,
    pub duration_ms: u64,
    pub modified: bool,
}
