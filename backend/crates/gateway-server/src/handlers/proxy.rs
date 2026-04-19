use axum::{
    body::Body,
    extract::{Request, State, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_auth::AuthMethod;
use gateway_core::{graphql, ip, rewrite, websocket};
use gateway_db::models::route::RouteProtocol;
use gateway_policy::error::PolicyError;
use gateway_tenant::context::TenantContext;

/// Main proxy handler: route matching, policy evaluation, protocol-aware forwarding.
pub async fn proxy_handler(
    State(state): State<Arc<AppState>>,
    ws: Option<WebSocketUpgrade>,
    req: Request<Body>,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let client_ip = ip::extract_client_ip(req.headers(), None);

    // 1. Resolve Tenant Context
    let tenant_ctx = match req.extensions().get::<TenantContext>() {
        Some(t) => t.clone(),
        None => return (StatusCode::UNAUTHORIZED, "Tenant resolution failed").into_response(),
    };
    let tenant_id = tenant_ctx.id();

    // 2. Resolve identity
    let auth_ctx = req.extensions().get::<RequireAuth>().map(|ra| &ra.0);
    let (api_key_id, user_id, api_key_rpm, daily_budget, monthly_budget): (
        Option<Uuid>, Option<Uuid>, Option<i32>, Option<f64>, Option<f64>,
    ) = match auth_ctx {
        Some(ctx) => match &ctx.method {
            AuthMethod::ApiKey { key_id, .. } => {
                let cached = state.api_key_cache.get("");
                match cached {
                    Some(c) => (Some(*key_id), Some(ctx.user_id), c.rate_limit_rpm, c.budget_daily, c.budget_monthly),
                    None => (Some(*key_id), Some(ctx.user_id), Some(60), None, None),
                }
            }
            AuthMethod::VirtualKey { vkey_id: _, rate_limit_rpm, budget_daily, budget_monthly, .. } => (
                None,
                Some(ctx.user_id),
                *rate_limit_rpm,
                *budget_daily,
                *budget_monthly,
            ),
            AuthMethod::Jwt { .. } => (None, Some(ctx.user_id), Some(60), None, None),
        },
        None => (None, None, Some(10), Some(0.0), Some(0.0)),
    };

    // 3. Match Route
    let routes = match state.route_repo.list_active_by_tenant(tenant_id).await {
        Ok(r) => r,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load routes").into_response(),
    };

    let matched_route = routes.into_iter().find(|r| path.starts_with(&r.path_pattern));
    let route = match matched_route {
        Some(r) => r,
        None => return (StatusCode::NOT_FOUND, "No matching route found").into_response(),
    };

    // 4. Path rewriting
    let rewritten_path = rewrite::rewrite_path(
        &path,
        &route.path_pattern,
        route.strip_prefix,
        &route.rewrite_rules,
    );

    // 5. GraphQL depth limiting
    if route.protocol == RouteProtocol::Graphql {
        let original_headers = req.headers().clone();
        if let Ok(body_bytes) = axum::body::to_bytes(req.into_body(), state.server_config.max_body_size).await {
            if let Ok(body_json) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
                if let Some(query) = graphql::extract_query(&body_json) {
                    let depth = graphql::query_depth(query);
                    let max_depth = 10;
                    if depth > max_depth {
                        return (StatusCode::BAD_REQUEST, format!("GraphQL query depth {depth} exceeds limit {max_depth}")).into_response();
                    }
                }
            }

            return forward_buffered(
                &state, method, &route, &rewritten_path, tenant_id,
                user_id, api_key_id, api_key_rpm, daily_budget, monthly_budget,
                &client_ip, original_headers, body_bytes,
            ).await;
        }
        return (StatusCode::BAD_REQUEST, "Failed to read request body").into_response();
    }

    // 6. WebSocket upgrade
    if websocket::is_websocket_upgrade(req.headers()) {
        if let Some(ws) = ws {
            let backend = match state.backend_repo.find_by_id(route.backend_id, tenant_id).await {
                Ok(b) => b,
                Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Backend misconfigured").into_response(),
            };
            let upstream_url = format!("{}{}", backend.endpoint.trim_end_matches('/'), rewritten_path);
            return ws.on_upgrade(move |socket| async move {
                if let Err(e) = websocket::relay_websocket(socket, &upstream_url).await {
                    error!("WebSocket relay error: {e}");
                }
            });
        }
    }

    // 7. Policy evaluation
    let current_body_size = req.headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);

    if let Err(e) = state.policy_engine.evaluate(
        &client_ip, tenant_id, user_id, api_key_id,
        api_key_rpm.unwrap_or(10) as u32,
        daily_budget.unwrap_or(0.0), monthly_budget.unwrap_or(0.0),
        0.001, None, current_body_size, None, None,
    ).await {
        return match e {
            PolicyError::RateLimited { .. } => (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response(),
            PolicyError::BudgetExceeded(msg) => (StatusCode::PAYMENT_REQUIRED, msg).into_response(),
            _ => (StatusCode::FORBIDDEN, e.to_string()).into_response(),
        };
    }

    // 8. Resolve backends and forward via GatewayEngine
    let backends = match state.backend_repo.list_active_by_tenant(tenant_id).await {
        Ok(b) => b.into_iter().filter(|b| b.id == route.backend_id).collect::<Vec<_>>(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load backends").into_response(),
    };

    if backends.is_empty() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "No backend configured for route").into_response();
    }

    // Build forwarding headers
    let mut forward_headers = reqwest::header::HeaderMap::new();
    for (name, value) in req.headers() {
        let name_str = name.as_str();
        if name_str != "host" && name_str != "authorization" && name_str != "x-api-key" && name_str != "connection" && name_str != "upgrade" {
            forward_headers.insert(name.clone(), value.clone());
        }
    }
    gateway_core::proxy::ProxyEngine::ensure_request_id(&mut forward_headers);

    // Buffer body for retry support
    let body_bytes = match axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::BAD_REQUEST, "Failed to read request body").into_response(),
    };

    match state.gateway_engine.forward_to_pool(&backends, method, &rewritten_path, forward_headers, body_bytes).await {
        Ok(resp) => {
            state.policy_engine.record_usage(tenant_id, 0.001);

            let status = resp.status();
            let mut res = Response::builder().status(status);
            for (name, value) in resp.headers() {
                res = res.header(name, value);
            }
            let full_body = Body::from_stream(resp.bytes_stream());
            res.body(full_body).unwrap().into_response()
        }
        Err(e) => {
            error!("Proxy error: {}", e);
            (StatusCode::BAD_GATEWAY, "Upstream connection failed").into_response()
        }
    }
}

/// Forward with a buffered body (used for GraphQL and small requests).
async fn forward_buffered(
    state: &Arc<AppState>,
    method: axum::http::Method,
    route: &gateway_db::models::route::Route,
    rewritten_path: &str,
    tenant_id: Uuid,
    user_id: Option<Uuid>,
    api_key_id: Option<Uuid>,
    api_key_rpm: Option<i32>,
    daily_budget: Option<f64>,
    monthly_budget: Option<f64>,
    client_ip: &str,
    original_headers: axum::http::HeaderMap,
    body_bytes: bytes::Bytes,
) -> Response {
    // Policy
    if let Err(e) = state.policy_engine.evaluate(
        client_ip, tenant_id, user_id, api_key_id,
        api_key_rpm.unwrap_or(10) as u32,
        daily_budget.unwrap_or(0.0), monthly_budget.unwrap_or(0.0),
        0.001, None, body_bytes.len(), None, None,
    ).await {
        return match e {
            PolicyError::RateLimited { .. } => (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded").into_response(),
            PolicyError::BudgetExceeded(msg) => (StatusCode::PAYMENT_REQUIRED, msg).into_response(),
            _ => (StatusCode::FORBIDDEN, e.to_string()).into_response(),
        };
    }

    let backends = match state.backend_repo.list_active_by_tenant(tenant_id).await {
        Ok(b) => b.into_iter().filter(|b| b.id == route.backend_id).collect::<Vec<_>>(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load backends").into_response(),
    };

    let mut forward_headers = reqwest::header::HeaderMap::new();
    for (name, value) in &original_headers {
        let name_str = name.as_str();
        if name_str != "host" && name_str != "authorization" && name_str != "x-api-key" {
            forward_headers.insert(name.clone(), value.clone());
        }
    }

    match state.gateway_engine.forward_to_pool(&backends, method, rewritten_path, forward_headers, body_bytes).await {
        Ok(resp) => {
            state.policy_engine.record_usage(tenant_id, 0.001);
            let status = resp.status();
            let mut res = Response::builder().status(status);
            for (name, value) in resp.headers() {
                res = res.header(name, value);
            }
            let full_body = Body::from_stream(resp.bytes_stream());
            res.body(full_body).unwrap().into_response()
        }
        Err(e) => {
            error!("Proxy error: {}", e);
            (StatusCode::BAD_GATEWAY, "Upstream connection failed").into_response()
        }
    }
}
