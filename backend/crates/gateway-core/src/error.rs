use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("No healthy backend available")]
    NoBackend,

    #[error("Backend timeout: {0}")]
    Timeout(String),

    #[error("Backend connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Backend returned error {status}: {body}")]
    BackendError { status: u16, body: String },

    #[error("Circuit breaker open for backend: {0}")]
    CircuitOpen(String),

    #[error("Protocol not supported: {0}")]
    UnsupportedProtocol(String),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("GraphQL query depth {depth} exceeds limit {limit}")]
    GraphqlDepthExceeded { depth: u32, limit: u32 },

    #[error("GraphQL introspection blocked")]
    GraphqlIntrospectionBlocked,

    #[error("gRPC error: {0}")]
    Grpc(String),

    #[error("Request body too large: {size} bytes (max {max})")]
    BodyTooLarge { size: usize, max: usize },

    #[error("Proxy error: {0}")]
    Internal(String),
}
