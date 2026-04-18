use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Invalid params: {0}")]
    InvalidParams(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Server not found: {0}")]
    ServerNotFound(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Rate limited")]
    RateLimited,
}

impl McpError {
    /// JSON-RPC error code per MCP spec.
    pub fn code(&self) -> i32 {
        match self {
            Self::ParseError(_) => -32700,
            Self::InvalidRequest(_) => -32600,
            Self::MethodNotFound(_) => -32601,
            Self::InvalidParams(_) => -32602,
            Self::Internal(_) => -32603,
            Self::ServerNotFound(_) => -32001,
            Self::ToolNotFound(_) => -32002,
            Self::ResourceNotFound(_) => -32003,
            Self::SessionNotFound(_) => -32004,
            Self::Transport(_) => -32005,
            Self::Timeout(_) => -32006,
            Self::Unauthorized(_) => -32007,
            Self::RateLimited => -32008,
        }
    }
}
