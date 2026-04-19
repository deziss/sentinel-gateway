use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Plugin init failed: {0}")]
    InitFailed(String),

    #[error("Plugin execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Plugin config invalid: {0}")]
    InvalidConfig(String),

    #[error("Plugin blocked request: {0}")]
    Blocked(String),

    #[error("Internal: {0}")]
    Internal(String),
}
