//! MCP (Model Context Protocol) HTTP handler.
//!
//! Implements the Streamable HTTP transport for MCP:
//! - `POST /api/v1/mcp` — JSON-RPC request handler (returns JSON or SSE)
//! - `GET /api/v1/mcp/servers` — List configured MCP backends
//! - `POST /api/v1/mcp/servers` — Register an MCP backend
//! - `DELETE /api/v1/mcp/servers/:id` — Remove an MCP backend
//! - `POST /api/v1/mcp/servers/:id/refresh` — Refresh backend discovery

use axum::{
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_mcp::protocol::JsonRpcRequest;

/// `POST /api/v1/mcp` — MCP JSON-RPC endpoint (Streamable HTTP transport).
///
/// AI agents send JSON-RPC requests here. The gateway processes them,
/// routes tool calls to appropriate upstream MCP servers, and returns results.
pub async fn handle_jsonrpc(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    headers: HeaderMap,
    Json(request): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    let session_id = headers
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let tenant_id = Some(auth.0.tenant_id);
    let user_id = Some(auth.0.user_id);

    // Touch session if exists
    if let Some(ref sid) = session_id {
        state.mcp_server.sessions().touch(sid);
    }

    let response = state.mcp_server
        .handle_request(request, session_id.as_deref(), tenant_id, user_id)
        .await;

    // Extract session ID from initialize response if present
    let mut resp_headers = HeaderMap::new();
    if let Some(ref result) = response.result {
        if let Some(sid) = result.get("_session_id").and_then(|v| v.as_str()) {
            let header_value: HeaderValue = sid.parse::<HeaderValue>()
                .ok()
                .unwrap_or(HeaderValue::from_static(""));
            resp_headers.insert(
                "Mcp-Session-Id",
                header_value,
            );
        }
    }

    // Clean internal _session_id from response
    let mut clean_response = response;
    if let Some(serde_json::Value::Object(ref mut map)) = clean_response.result {
        map.remove("_session_id");
    }

    (StatusCode::OK, resp_headers, Json(clean_response))
}

/// `GET /api/v1/mcp/servers` — List registered MCP backend servers.
pub async fn list_servers(
    State(state): State<Arc<AppState>>,
    Extension(_auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let backends = state.mcp_server.registry().list_backends();

    let servers: Vec<serde_json::Value> = backends
        .iter()
        .map(|b| {
            serde_json::json!({
                "id": b.id,
                "name": b.name,
                "url": b.url,
                "is_healthy": b.is_healthy,
                "tools_count": b.tools.len(),
                "resources_count": b.resources.len(),
                "prompts_count": b.prompts.len(),
            })
        })
        .collect();

    (StatusCode::OK, Json(serde_json::json!({
        "mcp_servers": servers,
        "total": servers.len(),
    })))
}

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterMcpServerRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    #[validate(url)]
    pub url: String,
}

/// `POST /api/v1/mcp/servers` — Register and connect to an upstream MCP server.
pub async fn register_server(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<RegisterMcpServerRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let id = Uuid::new_v4().to_string();

    match state.mcp_server.connect_backend(id.clone(), body.name.clone(), body.url.clone()).await {
        Ok(()) => {
            let backend = state.mcp_server.registry().get_backend(&id);

            state.audit_service.log(
                gateway_audit::events::AuditEvent::new(
                    auth.0.tenant_id,
                    gateway_audit::events::EventType::BackendCreated,
                    "mcp_server",
                )
                .with_user(auth.0.user_id)
                .with_resource_id(id.clone())
                .with_details(serde_json::json!({"name": body.name, "url": body.url})),
            );

            (StatusCode::CREATED, Json(serde_json::json!({
                "id": id,
                "name": body.name,
                "url": body.url,
                "tools_count": backend.as_ref().map(|b| b.tools.len()).unwrap_or(0),
                "resources_count": backend.as_ref().map(|b| b.resources.len()).unwrap_or(0),
                "prompts_count": backend.as_ref().map(|b| b.prompts.len()).unwrap_or(0),
            }))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, name = %body.name, "Failed to connect MCP server");
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                "error": format!("Failed to connect to MCP server: {e}")
            }))).into_response()
        }
    }
}

/// `DELETE /api/v1/mcp/servers/:id` — Disconnect and remove an MCP backend.
pub async fn remove_server(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    state.mcp_server.disconnect_backend(&id);

    state.audit_service.log(
        gateway_audit::events::AuditEvent::new(
            auth.0.tenant_id,
            gateway_audit::events::EventType::BackendDeleted,
            "mcp_server",
        )
        .with_user(auth.0.user_id)
        .with_resource_id(id),
    );

    StatusCode::NO_CONTENT
}

/// `POST /api/v1/mcp/servers/:id/refresh` — Refresh tool/resource discovery.
pub async fn refresh_server(
    State(state): State<Arc<AppState>>,
    Extension(_auth): Extension<RequireAuth>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.mcp_server.refresh_backend(&id).await {
        Ok(()) => {
            let backend = state.mcp_server.registry().get_backend(&id);
            (StatusCode::OK, Json(serde_json::json!({
                "status": "refreshed",
                "tools_count": backend.as_ref().map(|b| b.tools.len()).unwrap_or(0),
                "resources_count": backend.as_ref().map(|b| b.resources.len()).unwrap_or(0),
                "prompts_count": backend.as_ref().map(|b| b.prompts.len()).unwrap_or(0),
            }))).into_response()
        }
        Err(e) => {
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                "error": format!("Failed to refresh: {e}")
            }))).into_response()
        }
    }
}

/// `GET /api/v1/mcp/tools` — List all aggregated tools across all backends.
pub async fn list_tools(
    State(state): State<Arc<AppState>>,
    Extension(_auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let tools = state.mcp_server.registry().list_tools();
    (StatusCode::OK, Json(serde_json::json!({
        "tools": tools,
        "total": tools.len(),
    })))
}
