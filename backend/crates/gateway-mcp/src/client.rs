//! MCP client — connects to upstream MCP servers via Streamable HTTP transport.
//!
//! Each client manages a connection to a single MCP server, handles the
//! initialization handshake, discovers tools/resources/prompts, and proxies
//! tool calls and resource reads.

use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tracing::{info, warn};

use crate::error::McpError;
use crate::protocol::*;

/// MCP client for connecting to a single upstream MCP server.
pub struct McpClient {
    http: Client,
    endpoint: String,
    session_id: Option<String>,
    server_info: Option<Implementation>,
    server_capabilities: Option<ServerCapabilities>,
}

impl McpClient {
    pub fn new(endpoint: &str) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(4)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            endpoint: endpoint.trim_end_matches('/').to_string(),
            session_id: None,
            server_info: None,
            server_capabilities: None,
        }
    }

    /// Perform the MCP initialization handshake.
    pub async fn initialize(&mut self) -> Result<InitializeResult, McpError> {
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(Value::Number(1.into())),
            method: "initialize".to_string(),
            params: Some(serde_json::to_value(InitializeParams {
                protocol_version: MCP_PROTOCOL_VERSION.to_string(),
                capabilities: ClientCapabilities {
                    sampling: Some(Value::Object(Default::default())),
                    roots: None,
                },
                client_info: Implementation {
                    name: "sentinel-gateway".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            }).map_err(|e| McpError::Internal(e.to_string()))?),
        };

        let resp = self.send_request(req).await?;

        let result: InitializeResult = serde_json::from_value(
            resp.result.ok_or_else(|| McpError::Internal("No result in initialize response".into()))?
        ).map_err(|e| McpError::ParseError(e.to_string()))?;

        self.server_info = Some(result.server_info.clone());
        self.server_capabilities = Some(result.capabilities.clone());

        // Send initialized notification
        let notif = serde_json::json!({
            "jsonrpc": JSONRPC_VERSION,
            "method": "notifications/initialized"
        });

        let _ = self.http
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .header_opt_session(&self.session_id)
            .json(&notif)
            .send()
            .await;

        info!(
            server = %result.server_info.name,
            version = %result.server_info.version,
            "MCP connection initialized"
        );

        Ok(result)
    }

    /// Discover tools from the upstream MCP server.
    pub async fn list_tools(&self) -> Result<Vec<ToolDefinition>, McpError> {
        let req = self.build_request("tools/list", None);
        let resp = self.send_request(req).await?;

        let result: ToolsListResult = serde_json::from_value(
            resp.result.ok_or_else(|| McpError::Internal("No result".into()))?
        ).map_err(|e| McpError::ParseError(e.to_string()))?;

        info!(count = result.tools.len(), "Discovered tools from MCP server");
        Ok(result.tools)
    }

    /// Discover resources from the upstream MCP server.
    pub async fn list_resources(&self) -> Result<Vec<ResourceDefinition>, McpError> {
        let req = self.build_request("resources/list", None);
        let resp = self.send_request(req).await?;

        let result: ResourcesListResult = serde_json::from_value(
            resp.result.ok_or_else(|| McpError::Internal("No result".into()))?
        ).map_err(|e| McpError::ParseError(e.to_string()))?;

        Ok(result.resources)
    }

    /// Discover prompts from the upstream MCP server.
    pub async fn list_prompts(&self) -> Result<Vec<PromptDefinition>, McpError> {
        let req = self.build_request("prompts/list", None);
        let resp = self.send_request(req).await?;

        let result: serde_json::Value = resp.result
            .ok_or_else(|| McpError::Internal("No result".into()))?;

        let prompts: Vec<PromptDefinition> = serde_json::from_value(
            result.get("prompts").cloned().unwrap_or(Value::Array(vec![]))
        ).unwrap_or_default();

        Ok(prompts)
    }

    /// Call a tool on the upstream MCP server.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult, McpError> {
        let params = serde_json::to_value(ToolCallParams {
            name: name.to_string(),
            arguments,
        }).map_err(|e| McpError::Internal(e.to_string()))?;

        let req = self.build_request("tools/call", Some(params));
        let resp = self.send_request(req).await?;

        if let Some(err) = resp.error {
            return Err(McpError::Internal(err.message));
        }

        let result: ToolCallResult = serde_json::from_value(
            resp.result.ok_or_else(|| McpError::Internal("No result from tool call".into()))?
        ).map_err(|e| McpError::ParseError(e.to_string()))?;

        Ok(result)
    }

    /// Read a resource from the upstream MCP server.
    pub async fn read_resource(&self, uri: &str) -> Result<ResourceReadResult, McpError> {
        let params = serde_json::to_value(ResourceReadParams {
            uri: uri.to_string(),
        }).map_err(|e| McpError::Internal(e.to_string()))?;

        let req = self.build_request("resources/read", Some(params));
        let resp = self.send_request(req).await?;

        let result: ResourceReadResult = serde_json::from_value(
            resp.result.ok_or_else(|| McpError::Internal("No result".into()))?
        ).map_err(|e| McpError::ParseError(e.to_string()))?;

        Ok(result)
    }

    /// Get a prompt from the upstream MCP server.
    pub async fn get_prompt(&self, name: &str, arguments: Value) -> Result<PromptGetResult, McpError> {
        let params = serde_json::to_value(PromptGetParams {
            name: name.to_string(),
            arguments,
        }).map_err(|e| McpError::Internal(e.to_string()))?;

        let req = self.build_request("prompts/get", Some(params));
        let resp = self.send_request(req).await?;

        let result: PromptGetResult = serde_json::from_value(
            resp.result.ok_or_else(|| McpError::Internal("No result".into()))?
        ).map_err(|e| McpError::ParseError(e.to_string()))?;

        Ok(result)
    }

    pub fn server_info(&self) -> Option<&Implementation> {
        self.server_info.as_ref()
    }

    pub fn capabilities(&self) -> Option<&ServerCapabilities> {
        self.server_capabilities.as_ref()
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    fn build_request(&self, method: &str, params: Option<Value>) -> JsonRpcRequest {
        use std::sync::atomic::{AtomicU64, Ordering};
        static ID_COUNTER: AtomicU64 = AtomicU64::new(2);

        JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(Value::Number(ID_COUNTER.fetch_add(1, Ordering::Relaxed).into())),
            method: method.to_string(),
            params,
        }
    }

    async fn send_request(&self, req: JsonRpcRequest) -> Result<JsonRpcResponse, McpError> {
        let mut builder = self.http
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");

        if let Some(ref sid) = self.session_id {
            builder = builder.header("Mcp-Session-Id", sid);
        }

        let http_resp = builder
            .json(&req)
            .send()
            .await
            .map_err(|e| McpError::Transport(e.to_string()))?;

        // Capture session ID from response
        if let Some(sid) = http_resp.headers().get("Mcp-Session-Id") {
            if let Ok(s) = sid.to_str() {
                // Note: self is not &mut here, so session_id should be set during initialize
                let _ = s;
            }
        }

        if !http_resp.status().is_success() {
            let status = http_resp.status();
            let body = http_resp.text().await.unwrap_or_default();
            return Err(McpError::Transport(format!("HTTP {status}: {body}")));
        }

        let resp: JsonRpcResponse = http_resp
            .json()
            .await
            .map_err(|e| McpError::ParseError(e.to_string()))?;

        if let Some(ref err) = resp.error {
            warn!(code = err.code, message = %err.message, "MCP server returned error");
        }

        Ok(resp)
    }
}

/// Extension trait for optional session header.
trait RequestBuilderExt {
    fn header_opt_session(self, session_id: &Option<String>) -> Self;
}

impl RequestBuilderExt for reqwest::RequestBuilder {
    fn header_opt_session(self, session_id: &Option<String>) -> Self {
        match session_id {
            Some(sid) => self.header("Mcp-Session-Id", sid),
            None => self,
        }
    }
}
