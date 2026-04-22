pub mod health;

use axum::Router;
use std::sync::Arc;
use crate::state::AppState;
use gateway_auth::middleware::AuthState;

pub fn api_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    let tenant_middleware_state = Arc::new(gateway_tenant::middleware::TenantMiddlewareState {
        service: state.tenant_service.clone(),
        saas_mode: state.server_config.saas_mode
            || state.deployment_mode == gateway_license::DeploymentMode::Local,
    });

    let auth_state = build_auth_state(&state);

    let tls_config = Arc::new(gateway_telemetry::middleware::TlsEnforcement {
        require_tls: state.server_config.require_tls,
        trust_forwarded_proto: state.server_config.trust_forwarded_proto,
    });

    Router::new()
        .nest("/api/v1", v1_routes(state.clone()))
        .fallback(crate::handlers::proxy::proxy_handler)
        .layer(axum::middleware::from_fn_with_state(
            auth_state,
            gateway_auth::middleware::optional_auth_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            tenant_middleware_state,
            gateway_tenant::middleware::tenant_middleware
        ))
        .layer(axum::middleware::from_fn(
            gateway_telemetry::middleware::telemetry_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            tls_config,
            gateway_telemetry::middleware::tls_enforcement_middleware,
        ))
}

fn build_auth_state(state: &Arc<AppState>) -> Arc<AuthState> {
    Arc::new(AuthState {
        jwt: state.jwt.clone(),
        token_blacklist: state.token_blacklist.clone(),
        api_key_cache: state.api_key_cache.clone(),
        api_key_repo: state.api_key_repo.clone(),
        user_repo: state.user_repo.clone(),
        virtual_key_repo: state.virtual_key_repo.clone(),
    })
}

fn v1_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    let auth_state = build_auth_state(&state);

    // ── Open Routes (No authentication required) ───────────────────────────
    let open_routes = Router::new()
        .route("/auth/login", axum::routing::post(crate::handlers::auth::login))
        .route("/auth/refresh", axum::routing::post(crate::handlers::auth::refresh))
        .route("/auth/sso/:slug/authorize", axum::routing::get(crate::handlers::sso_auth::authorize))
        .route("/auth/sso/:slug/callback", axum::routing::get(crate::handlers::sso_auth::callback));

    // ── SuperAdmin Routes ──────────────────────────────────────────────────
    let super_admin_routes = Router::new()
        .route("/tenants", axum::routing::get(crate::handlers::tenants::list).post(crate::handlers::tenants::create))
        .route("/tenants/:id", axum::routing::get(crate::handlers::tenants::get)
            .put(crate::handlers::tenants::update)
            .delete(crate::handlers::tenants::delete))
        .route("/tenants/:id/license", axum::routing::get(crate::handlers::admin::get_tenant_license)
            .post(crate::handlers::admin::set_tenant_license))
        .route("/admin/slow-queries", axum::routing::get(crate::handlers::admin::slow_queries))
        .route("/admin/slow-queries/reset", axum::routing::post(crate::handlers::admin::reset_slow_queries))
        .route("/admin/config/reload", axum::routing::post(crate::handlers::admin::reload_config))
        .route("/organizations", axum::routing::get(crate::handlers::organizations::list)
            .post(crate::handlers::organizations::create))
        .route("/organizations/:id", axum::routing::get(crate::handlers::organizations::get)
            .delete(crate::handlers::organizations::delete))
        .route("/organizations/:id/tenants", axum::routing::get(crate::handlers::organizations::list_tenants))
        .route("/organizations/:id/tenants/:tenant_id", axum::routing::post(crate::handlers::organizations::assign_tenant))
        .route_layer(axum::middleware::from_fn(|req, next| {
            gateway_auth::middleware::role_gate(req, next, gateway_auth::Role::SuperAdmin)
        }));

    // ── TenantAdmin Routes (Management features) ───────────────────────────
    let tenant_admin_routes = Router::new()
        .route("/backends", axum::routing::get(crate::handlers::backends::list).post(crate::handlers::backends::create))
        .route("/backends/:id", axum::routing::get(crate::handlers::backends::get)
            .put(crate::handlers::backends::update)
            .delete(crate::handlers::backends::delete))
        .route("/api-keys", axum::routing::get(crate::handlers::api_keys::list).post(crate::handlers::api_keys::create))
        .route("/api-keys/:id", axum::routing::delete(crate::handlers::api_keys::revoke))
        .route("/users", axum::routing::get(crate::handlers::users::list).post(crate::handlers::users::invite))
        .route("/users/:id", axum::routing::get(crate::handlers::users::get)
            .put(crate::handlers::users::update)
            .delete(crate::handlers::users::deactivate))
        .route("/routes", axum::routing::get(crate::handlers::proxy_routes::list).post(crate::handlers::proxy_routes::create))
        .route("/routes/:id", axum::routing::delete(crate::handlers::proxy_routes::delete))
        .route("/audit-logs", axum::routing::get(crate::handlers::audit_logs::list))
        .route("/webhooks", axum::routing::get(crate::handlers::webhooks::list).post(crate::handlers::webhooks::create))
        .route("/webhooks/:id", axum::routing::delete(crate::handlers::webhooks::delete))
        .route("/webhooks/:id/test", axum::routing::post(crate::handlers::webhooks::test))
        .route("/webhooks/failures", axum::routing::get(crate::handlers::webhooks::list_failures))
        .route("/webhooks/failures/:id/retry", axum::routing::post(crate::handlers::webhooks::retry_failure))
        .route("/teams", axum::routing::get(crate::handlers::teams::list).post(crate::handlers::teams::create))
        .route("/teams/:id", axum::routing::get(crate::handlers::teams::get).delete(crate::handlers::teams::delete))
        .route("/teams/:id/members", axum::routing::get(crate::handlers::teams::list_members).post(crate::handlers::teams::add_member))
        .route("/teams/:id/members/:user_id", axum::routing::delete(crate::handlers::teams::remove_member))
        .route("/virtual-keys", axum::routing::get(crate::handlers::virtual_keys::list).post(crate::handlers::virtual_keys::create))
        .route("/virtual-keys/:id", axum::routing::delete(crate::handlers::virtual_keys::revoke))
        .route("/llm-logs", axum::routing::get(crate::handlers::llm_logs::list))
        .route("/pricing", axum::routing::get(crate::handlers::pricing::list).put(crate::handlers::pricing::upsert))
        .route("/pricing/:model", axum::routing::delete(crate::handlers::pricing::delete))
        .route("/usage", axum::routing::get(crate::handlers::usage::summary))
        .route("/sync/status", axum::routing::get(crate::handlers::sync::status))
        .route("/sync/register", axum::routing::post(crate::handlers::sync::register))
        .route("/sync/push", axum::routing::post(crate::handlers::sync::push))
        .route("/sync/pull", axum::routing::post(crate::handlers::sync::pull))
        .route("/sync/unlink", axum::routing::post(crate::handlers::sync::unlink))
        .route("/mcp", axum::routing::post(crate::handlers::mcp::handle_jsonrpc))
        .route("/mcp/servers", axum::routing::get(crate::handlers::mcp::list_servers)
            .post(crate::handlers::mcp::register_server))
        .route("/mcp/servers/:id", axum::routing::delete(crate::handlers::mcp::remove_server))
        .route("/mcp/servers/:id/refresh", axum::routing::post(crate::handlers::mcp::refresh_server))
        .route("/mcp/tools", axum::routing::get(crate::handlers::mcp::list_tools))
        .route("/guardrails", axum::routing::get(crate::handlers::guardrails::list)
            .post(crate::handlers::guardrails::create))
        .route("/guardrails/:id", axum::routing::get(crate::handlers::guardrails::get)
            .put(crate::handlers::guardrails::update)
            .delete(crate::handlers::guardrails::delete))
        .route("/guardrails/test", axum::routing::post(crate::handlers::guardrails::test_pipeline))
        .route("/prompts", axum::routing::get(crate::handlers::prompts::list_names)
            .post(crate::handlers::prompts::create))
        .route("/prompts/:name/versions", axum::routing::get(crate::handlers::prompts::list_versions))
        .route("/prompts/:name/versions/:version", axum::routing::get(crate::handlers::prompts::get_version)
            .delete(crate::handlers::prompts::delete_version))
        .route("/prompts/:name/deploy", axum::routing::post(crate::handlers::prompts::deploy))
        .route("/prompts/:name/deployments", axum::routing::get(crate::handlers::prompts::list_deployments))
        .route("/prompts/:name/resolve", axum::routing::post(crate::handlers::prompts::resolve))
        .route("/settings", axum::routing::get(crate::handlers::settings::list).put(crate::handlers::settings::update))
        .route("/settings/:key", axum::routing::delete(crate::handlers::settings::delete_key))
        .route("/sso/providers", axum::routing::get(crate::handlers::sso_auth::list_providers)
            .post(crate::handlers::sso_auth::create_provider))
        .route("/sso/providers/:id", axum::routing::delete(crate::handlers::sso_auth::delete_provider))
        .route_layer(axum::middleware::from_fn(|req, next| {
            gateway_auth::middleware::role_gate(req, next, gateway_auth::Role::TenantAdmin)
        }));

    // ── Regular User Routes (Authenticated but no specific admin role) ─────
    let authenticated_routes = Router::new()
        .route("/auth/logout", axum::routing::post(crate::handlers::auth::logout))
        .route("/license/status", axum::routing::get(crate::handlers::license::status))
        .route("/license/features", axum::routing::get(crate::handlers::license::features))
        .route("/license/activate", axum::routing::post(crate::handlers::license::activate))
        .route("/v1/chat/completions", axum::routing::post(crate::handlers::llm::chat_completions))
        .route("/v1/completions", axum::routing::post(crate::handlers::llm::completions))
        .route("/v1/embeddings", axum::routing::post(crate::handlers::llm::embeddings))
        .route("/v1/models", axum::routing::get(crate::handlers::llm::list_models))
        .route("/v1/images/generations", axum::routing::post(crate::handlers::llm::images_generations))
        .route("/v1/images/edits", axum::routing::post(crate::handlers::llm::images_edits))
        .route("/v1/audio/transcriptions", axum::routing::post(crate::handlers::llm::audio_transcriptions))
        .route("/v1/audio/speech", axum::routing::post(crate::handlers::llm::audio_speech))
        .route("/feedback", axum::routing::get(crate::handlers::feedback::list)
            .post(crate::handlers::feedback::submit))
        .route("/feedback/stats", axum::routing::get(crate::handlers::feedback::stats));

    // Combine all protected routes and apply authentication
    let protected_routes = Router::new()
        .merge(authenticated_routes)
        .merge(tenant_admin_routes)
        .merge(super_admin_routes)
        .route_layer(axum::middleware::from_fn_with_state(
            auth_state,
            gateway_auth::middleware::auth_middleware
        ));

    open_routes.merge(protected_routes)
}

