//! MCP server handler — acts as an MCP server to downstream AI agents.
//!
//! Receives JSON-RPC requests from AI agents (Claude, custom agents) and
//! routes them to the appropriate upstream MCP backend via the registry.
//! Handles initialization, tool discovery, tool execution, and resource access.

use serde_json::Value;
use std::sync::Arc;
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::client::McpClient;
use crate::error::McpError;
use crate::protocol::*;
use crate::registry::McpRegistry;
use crate::session::SessionStore;

/// The MCP gateway server that handles incoming JSON-RPC requests.
pub struct McpServer {
    registry: Arc<McpRegistry>,
    sessions: Arc<SessionStore>,
    clients: Arc<dashmap::DashMap<String, McpClient>>,
}

impl McpServer {
    pub fn new(registry: Arc<McpRegistry>, sessions: Arc<SessionStore>) -> Self {
        Self {
            registry,
            sessions,
            clients: Arc::new(dashmap::DashMap::new()),
        }
    }

    /// Handle an incoming JSON-RPC request and return a response.
    pub async fn handle_request(
        &self,
        request: JsonRpcRequest,
        session_id: Option<&str>,
        tenant_id: Option<Uuid>,
        user_id: Option<Uuid>,
    ) -> JsonRpcResponse {
        let id = request.id.clone().unwrap_or(Value::Null);

        match request.method.as_str() {
            "initialize" => self.handle_initialize(id, request.params, tenant_id, user_id).await,
            "tools/list" => self.handle_tools_list(id, session_id).await,
            "tools/call" => self.handle_tools_call(id, request.params, session_id).await,
            "resources/list" => self.handle_resources_list(id, session_id).await,
            "resources/read" => self.handle_resources_read(id, request.params, session_id).await,
            "prompts/list" => self.handle_prompts_list(id, session_id).await,
            "prompts/get" => self.handle_prompts_get(id, request.params, session_id).await,
            "ping" => JsonRpcResponse::success(id, serde_json::json!({})),
            method if method.starts_with("notifications/") => {
                // Notifications don't get responses, but we return empty for HTTP
                JsonRpcResponse::success(id, serde_json::json!({}))
            }
            method => {
                warn!(method, "Unknown MCP method");
                JsonRpcResponse::error(id, -32601, format!("Method not found: {method}"))
            }
        }
    }

    // ── Initialize ───────────────────────────────────────────────────────────

    async fn handle_initialize(
        &self,
        id: Value,
        params: Option<Value>,
        tenant_id: Option<Uuid>,
        user_id: Option<Uuid>,
    ) -> JsonRpcResponse {
        let init_params: InitializeParams = match params
            .and_then(|p| serde_json::from_value(p).ok())
        {
            Some(p) => p,
            None => return JsonRpcResponse::error(id, -32602, "Invalid initialize params"),
        };

        let server_caps = ServerCapabilities {
            tools: Some(ToolsCapability { list_changed: true }),
            resources: Some(ResourcesCapability { subscribe: false, list_changed: true }),
            prompts: Some(PromptsCapability { list_changed: true }),
            logging: Some(Value::Object(Default::default())),
        };

        let session = self.sessions.create(
            init_params.client_info.clone(),
            init_params.capabilities,
            server_caps.clone(),
            tenant_id,
            user_id,
        );

        info!(
            session_id = %session.id,
            client = %init_params.client_info.name,
            version = %init_params.client_info.version,
            "MCP session initialized"
        );

        let result = InitializeResult {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: server_caps,
            server_info: Implementation {
                name: "sentinel-gateway-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some(
                "Sentinel Gateway MCP Server. Tools are namespaced by backend: \
                 {backend}__{tool_name}. Use tools/list to discover available tools."
                    .to_string(),
            ),
        };

        let mut resp = JsonRpcResponse::success(
            id,
            serde_json::to_value(result).unwrap_or(Value::Null),
        );

        // The session ID would be returned via Mcp-Session-Id header in the HTTP layer
        // We attach it to the response metadata for the handler to extract
        if let Some(ref mut r) = resp.result {
            if let Value::Object(ref mut map) = r {
                map.insert("_session_id".to_string(), Value::String(session.id));
            }
        }

        resp
    }

    // ── Tools ────────────────────────────────────────────────────────────────

    async fn handle_tools_list(&self, id: Value, session_id: Option<&str>) -> JsonRpcResponse {
        if session_id.is_none() {
            return JsonRpcResponse::error(id, -32004, "Session required — send initialize first");
        }

        let tools = self.registry.list_tools();

        let result = ToolsListResult {
            tools,
            cursor: None,
        };

        JsonRpcResponse::success(
            id,
            serde_json::to_value(result).unwrap_or(Value::Null),
        )
    }

    async fn handle_tools_call(
        &self,
        id: Value,
        params: Option<Value>,
        session_id: Option<&str>,
    ) -> JsonRpcResponse {
        if session_id.is_none() {
            return JsonRpcResponse::error(id, -32004, "Session required");
        }

        let call_params: ToolCallParams = match params
            .and_then(|p| serde_json::from_value(p).ok())
        {
            Some(p) => p,
            None => return JsonRpcResponse::error(id, -32602, "Invalid tool call params"),
        };

        // Resolve namespaced tool to backend + original name
        let (backend_id, original_name) = match self.registry.resolve_tool(&call_params.name) {
            Some(r) => r,
            None => return JsonRpcResponse::error(
                id, -32002,
                format!("Tool not found: {}", call_params.name),
            ),
        };

        // Get the client for this backend
        let client = match self.clients.get(&backend_id) {
            Some(c) => c,
            None => return JsonRpcResponse::error(
                id, -32001,
                format!("Backend not connected: {backend_id}"),
            ),
        };

        info!(
            tool = %call_params.name,
            backend = %backend_id,
            "Proxying MCP tool call"
        );

        match client.call_tool(&original_name, call_params.arguments).await {
            Ok(result) => JsonRpcResponse::success(
                id,
                serde_json::to_value(result).unwrap_or(Value::Null),
            ),
            Err(e) => {
                error!(error = %e, tool = %call_params.name, "MCP tool call failed");
                JsonRpcResponse::error(id, e.code(), e.to_string())
            }
        }
    }

    // ── Resources ────────────────────────────────────────────────────────────

    async fn handle_resources_list(&self, id: Value, session_id: Option<&str>) -> JsonRpcResponse {
        if session_id.is_none() {
            return JsonRpcResponse::error(id, -32004, "Session required");
        }

        let resources = self.registry.list_resources();

        let result = ResourcesListResult {
            resources,
            cursor: None,
        };

        JsonRpcResponse::success(
            id,
            serde_json::to_value(result).unwrap_or(Value::Null),
        )
    }

    async fn handle_resources_read(
        &self,
        id: Value,
        params: Option<Value>,
        session_id: Option<&str>,
    ) -> JsonRpcResponse {
        if session_id.is_none() {
            return JsonRpcResponse::error(id, -32004, "Session required");
        }

        let read_params: ResourceReadParams = match params
            .and_then(|p| serde_json::from_value(p).ok())
        {
            Some(p) => p,
            None => return JsonRpcResponse::error(id, -32602, "Invalid resource read params"),
        };

        let (backend_id, original_uri) = match self.registry.resolve_resource(&read_params.uri) {
            Some(r) => r,
            None => return JsonRpcResponse::error(
                id, -32003,
                format!("Resource not found: {}", read_params.uri),
            ),
        };

        let client = match self.clients.get(&backend_id) {
            Some(c) => c,
            None => return JsonRpcResponse::error(id, -32001, "Backend not connected"),
        };

        match client.read_resource(&original_uri).await {
            Ok(result) => JsonRpcResponse::success(
                id,
                serde_json::to_value(result).unwrap_or(Value::Null),
            ),
            Err(e) => JsonRpcResponse::error(id, e.code(), e.to_string()),
        }
    }

    // ── Prompts ──────────────────────────────────────────────────────────────

    async fn handle_prompts_list(&self, id: Value, session_id: Option<&str>) -> JsonRpcResponse {
        if session_id.is_none() {
            return JsonRpcResponse::error(id, -32004, "Session required");
        }

        let prompts = self.registry.list_prompts();

        let result = serde_json::json!({
            "prompts": prompts,
        });

        JsonRpcResponse::success(id, result)
    }

    async fn handle_prompts_get(
        &self,
        id: Value,
        params: Option<Value>,
        session_id: Option<&str>,
    ) -> JsonRpcResponse {
        if session_id.is_none() {
            return JsonRpcResponse::error(id, -32004, "Session required");
        }

        let get_params: PromptGetParams = match params
            .and_then(|p| serde_json::from_value(p).ok())
        {
            Some(p) => p,
            None => return JsonRpcResponse::error(id, -32602, "Invalid prompt get params"),
        };

        let (backend_id, original_name) = match self.registry.resolve_prompt(&get_params.name) {
            Some(r) => r,
            None => return JsonRpcResponse::error(
                id, -32003,
                format!("Prompt not found: {}", get_params.name),
            ),
        };

        let client = match self.clients.get(&backend_id) {
            Some(c) => c,
            None => return JsonRpcResponse::error(id, -32001, "Backend not connected"),
        };

        match client.get_prompt(&original_name, get_params.arguments).await {
            Ok(result) => JsonRpcResponse::success(
                id,
                serde_json::to_value(result).unwrap_or(Value::Null),
            ),
            Err(e) => JsonRpcResponse::error(id, e.code(), e.to_string()),
        }
    }

    // ── Backend Management ───────────────────────────────────────────────────

    /// Connect to an upstream MCP server and discover its primitives.
    pub async fn connect_backend(
        &self,
        id: String,
        name: String,
        url: String,
    ) -> Result<(), McpError> {
        let mut client = McpClient::new(&url);

        let init_result = client.initialize().await?;

        let tools = if init_result.capabilities.tools.is_some() {
            client.list_tools().await.unwrap_or_default()
        } else {
            vec![]
        };

        let resources = if init_result.capabilities.resources.is_some() {
            client.list_resources().await.unwrap_or_default()
        } else {
            vec![]
        };

        let prompts = if init_result.capabilities.prompts.is_some() {
            client.list_prompts().await.unwrap_or_default()
        } else {
            vec![]
        };

        info!(
            backend = %name,
            tools = tools.len(),
            resources = resources.len(),
            prompts = prompts.len(),
            "MCP backend connected and discovered"
        );

        let backend = crate::registry::McpBackend {
            id: id.clone(),
            name,
            url,
            tools,
            resources,
            prompts,
            is_healthy: true,
        };

        self.registry.register_backend(backend);
        self.clients.insert(id, client);

        Ok(())
    }

    /// Disconnect from an upstream MCP server.
    pub fn disconnect_backend(&self, backend_id: &str) {
        self.registry.remove_backend(backend_id);
        self.clients.remove(backend_id);
    }

    /// Refresh tool/resource discovery for a connected backend.
    pub async fn refresh_backend(&self, backend_id: &str) -> Result<(), McpError> {
        let client = self.clients.get(backend_id)
            .ok_or_else(|| McpError::ServerNotFound(backend_id.to_string()))?;

        let tools = client.list_tools().await.unwrap_or_default();
        let resources = client.list_resources().await.unwrap_or_default();
        let prompts = client.list_prompts().await.unwrap_or_default();

        if let Some(mut backend) = self.registry.get_backend(backend_id).map(|b| b) {
            backend.tools = tools;
            backend.resources = resources;
            backend.prompts = prompts;
            self.registry.register_backend(backend);
        }

        Ok(())
    }

    pub fn registry(&self) -> &McpRegistry {
        &self.registry
    }

    pub fn sessions(&self) -> &SessionStore {
        &self.sessions
    }
}
