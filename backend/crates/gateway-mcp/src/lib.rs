//! # gateway-mcp
//!
//! Model Context Protocol (MCP) gateway crate for Sentinel Gateway.
//!
//! Acts as a **dual-role MCP proxy**:
//! - **MCP Server** to downstream AI agents (exposes aggregated tools/resources)
//! - **MCP Client** to upstream MCP servers (connects, discovers, proxies calls)
//!
//! ## Architecture
//!
//! ```text
//! AI Agent ──→ Gateway (MCP Server) ──→ Registry ──→ MCP Server A (tools: github__)
//!                                                ──→ MCP Server B (tools: slack__)
//!                                                ──→ MCP Server C (tools: db__)
//! ```
//!
//! Tools are namespaced by backend: `{backend_name}__{tool_name}` to avoid collisions.
//! Resources use URI namespacing: `mcp://{backend_name}/{original_uri}`.

pub mod protocol;
pub mod error;
pub mod session;
pub mod registry;
pub mod client;
pub mod server;

pub use error::McpError;
pub use protocol::*;
pub use session::SessionStore;
pub use registry::{McpRegistry, McpBackend};
pub use client::McpClient;
pub use server::McpServer;
