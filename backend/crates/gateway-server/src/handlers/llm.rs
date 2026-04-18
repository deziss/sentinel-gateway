use axum::{Json, extract::State, http::StatusCode, response::IntoResponse, Extension};
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;

use crate::state::AppState;
use gateway_auth::middleware::RequireAuth;
use gateway_auth::AuthMethod;
use gateway_llm::{CostCalculator, ProviderAdapter, TokenCounter};
use gateway_db::models::usage_record::CreateUsageRecord;
use gateway_tenant::TenantContext;

/// `POST /v1/chat/completions` — OpenAI-compatible chat endpoint.
///
/// **Extension**: In addition to OpenAI's format, accepts a `prompt_ref` object to
/// inject a managed prompt from the prompt registry as the system message:
/// ```json
/// {
///   "prompt_ref": { "name": "customer_support", "label": "prod", "variables": {"brand": "Acme"} },
///   "model": "gpt-4o",
///   "messages": [{"role": "user", "content": "What's your return policy?"}]
/// }
/// ```
/// On resolution: the prompt content (rendered with variables) becomes the system
/// message. `default_model` and `model_prefs` fill in fields the client didn't set.
pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    _tenant_ctx: Option<Extension<TenantContext>>,
    Json(mut body): Json<Value>,
) -> impl IntoResponse {
    let started = Instant::now();
    let tenant_id = auth.0.tenant_id;
    let user_id = auth.0.user_id;
    let api_key_id = match &auth.0.method {
        AuthMethod::ApiKey { key_id, .. } => Some(*key_id),
        _ => None,
    };

    // 0. Resolve `prompt_ref` if present — injects system message and applies defaults.
    if let Some(prompt_ref) = body.get("prompt_ref").cloned() {
        let name = prompt_ref.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let label = prompt_ref.get("label").and_then(|v| v.as_str()).map(|s| s.to_string());
        let variables = prompt_ref.get("variables").cloned().unwrap_or(serde_json::json!({}));

        if !name.is_empty() {
            match state.prompt_repo.resolve(tenant_id, &name, label.as_deref()).await {
                Ok(Some(prompt)) => {
                    let rendered = crate::handlers::prompts::render_template(&prompt.content, &variables);
                    // Inject as system message at position 0 (replacing existing system, if any)
                    if let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) {
                        let already_system = messages.first()
                            .and_then(|m| m.get("role"))
                            .and_then(|r| r.as_str())
                            == Some("system");
                        if already_system {
                            messages[0] = serde_json::json!({"role": "system", "content": rendered});
                        } else {
                            messages.insert(0, serde_json::json!({"role": "system", "content": rendered}));
                        }
                    }
                    // Apply default_model if client didn't specify
                    if body.get("model").and_then(|m| m.as_str()).is_none() {
                        if let Some(ref default_model) = prompt.default_model {
                            if let Some(obj) = body.as_object_mut() {
                                obj.insert("model".into(), Value::String(default_model.clone()));
                            }
                        }
                    }
                    // Apply model_prefs (temperature, max_tokens, etc.) if client didn't override
                    if let Some(prefs) = prompt.model_prefs.as_object() {
                        if let Some(body_obj) = body.as_object_mut() {
                            for (k, v) in prefs {
                                body_obj.entry(k.clone()).or_insert(v.clone());
                            }
                        }
                    }
                }
                Ok(None) => {
                    return (StatusCode::NOT_FOUND, Json(serde_json::json!({
                        "error": { "message": format!("Prompt '{name}' not found"), "type": "prompt_not_found" }
                    }))).into_response();
                }
                Err(e) => {
                    tracing::error!(error = %e, prompt = %name, "Failed to resolve prompt");
                    return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                        "error": { "message": "Failed to resolve prompt", "type": "internal_error" }
                    }))).into_response();
                }
            }
            if let Some(obj) = body.as_object_mut() {
                obj.remove("prompt_ref");
            }
        }
    }

    // 1. Extract model name
    let model = body.get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("gpt-4o")
        .to_string();

    // 2. Route to provider
    let provider = match state.llm_router.select(&model) {
        Ok(p) => p,
        Err(e) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "error": { "message": e.to_string(), "type": "model_not_found" }
            }))).into_response();
        }
    };

    // 3. Count input tokens
    let tokens_in = if let Some(messages) = body.get("messages").and_then(|m| m.as_array()) {
        TokenCounter::count_messages(messages) as u64
    } else {
        0
    };

    // 4. Adapt request to provider format
    let adapted_body = match ProviderAdapter::adapt_request(&provider.provider_type, &body) {
        Ok(b) => b,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": { "message": e.to_string(), "type": "invalid_request" }
            }))).into_response();
        }
    };

    // 5. Build forwarding request with trace context propagation
    let url = provider.chat_url();
    let mut forward_headers = reqwest::header::HeaderMap::new();
    forward_headers.insert("content-type", "application/json".parse().unwrap());
    if let Some((header, value)) = provider.auth_header() {
        if let (Ok(name), Ok(val)) = (
            reqwest::header::HeaderName::from_bytes(header.as_bytes()),
            reqwest::header::HeaderValue::from_str(&value),
        ) {
            forward_headers.insert(name, val);
        }
    }
    // Inject W3C trace context for distributed tracing
    gateway_telemetry::inject_trace_context(&mut forward_headers);

    let req = reqwest::Client::new()
        .post(&url)
        .headers(forward_headers)
        .json(&adapted_body);

    // 6. Forward to provider
    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            let latency = started.elapsed().as_millis() as i64;
            // Record failed usage
            let _ = state.usage_record_repo.create(CreateUsageRecord {
                tenant_id,
                user_id: Some(user_id),
                api_key_id,
                backend_id: provider.id,
                model: Some(model),
                tokens_input: tokens_in as i64,
                tokens_output: 0,
                cost_usd: 0.0,
                latency_ms: latency,
                status_code: 502,
                error: Some(e.to_string()),
            }).await;

            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                "error": { "message": format!("Provider error: {e}"), "type": "provider_error" }
            }))).into_response();
        }
    };

    let status = resp.status();
    let resp_body: Value = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                "error": { "message": format!("Invalid provider response: {e}"), "type": "provider_error" }
            }))).into_response();
        }
    };

    let latency = started.elapsed().as_millis() as i64;

    if !status.is_success() {
        let _ = state.usage_record_repo.create(CreateUsageRecord {
            tenant_id,
            user_id: Some(user_id),
            api_key_id,
            backend_id: provider.id,
            model: Some(model),
            tokens_input: tokens_in as i64,
            tokens_output: 0,
            cost_usd: 0.0,
            latency_ms: latency,
            status_code: status.as_u16() as i32,
            error: Some(resp_body.to_string()),
        }).await;

        return (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            Json(resp_body)).into_response();
    }

    // 7. Adapt response back to OpenAI format
    let resolved_model = state.llm_router.resolve_alias(&model);
    let openai_resp = ProviderAdapter::adapt_response(&provider.provider_type, &resp_body, &resolved_model);

    // 8. Extract output tokens (from response or count)
    let tokens_out = openai_resp.get("usage")
        .and_then(|u| u.get("completion_tokens"))
        .and_then(|t| t.as_u64())
        .unwrap_or(0);

    // 9. Calculate cost
    let cost = CostCalculator::calculate(&resolved_model, tokens_in, tokens_out);

    // 10. Record metrics
    let tenant_str = tenant_id.to_string();
    state.metrics.record_proxy_request(
        &tenant_str, &provider.name, "200",
        started.elapsed().as_secs_f64(), &resolved_model,
    );
    state.metrics.record_tokens(&tenant_str, &resolved_model, tokens_in, tokens_out);
    state.metrics.record_cost(&tenant_str, &resolved_model, cost);

    tracing::info!(
        tenant_id = %tenant_id,
        user_id = %user_id,
        model = %resolved_model,
        provider = %provider.provider_type,
        tokens_in = tokens_in,
        tokens_out = tokens_out,
        cost_usd = cost,
        latency_ms = latency,
        "LLM request completed"
    );

    // 11. Push to optional observability export (Langfuse / Helicone).
    //     Fire-and-forget — no-op if exporter is disabled.
    if state.observability_exporter.is_enabled() {
        use crate::observability_export::TraceEvent;
        state.observability_exporter.push(TraceEvent {
            trace_id: uuid::Uuid::new_v4().to_string(),
            tenant_id: Some(tenant_id),
            user_id: Some(user_id),
            api_key_id,
            model: resolved_model.clone(),
            provider: provider.provider_type.to_string(),
            request: body.clone(),
            response: openai_resp.clone(),
            status_code: 200,
            latency_ms: latency as u64,
            prompt_tokens: tokens_in,
            completion_tokens: tokens_out,
            cost_usd: cost,
            started_at: chrono::Utc::now() - chrono::Duration::milliseconds(latency),
        });
    }

    // 12. Record usage in DB (fire-and-forget)
    let repo = state.usage_record_repo.clone();
    let record = CreateUsageRecord {
        tenant_id,
        user_id: Some(user_id),
        api_key_id,
        backend_id: provider.id,
        model: Some(resolved_model),
        tokens_input: tokens_in as i64,
        tokens_output: tokens_out as i64,
        cost_usd: cost,
        latency_ms: latency,
        status_code: 200,
        error: None,
    };
    tokio::spawn(async move {
        if let Err(e) = repo.create(record).await {
            tracing::warn!("Failed to record LLM usage: {e}");
        }
    });

    (StatusCode::OK, Json(openai_resp)).into_response()
}

/// `POST /v1/completions` — legacy completions endpoint (proxied as chat).
pub async fn completions(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    _tenant_ctx: Option<Extension<TenantContext>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Convert legacy completions to chat format
    let prompt = body.get("prompt").and_then(|p| p.as_str()).unwrap_or("");
    let chat_body = serde_json::json!({
        "model": body.get("model").unwrap_or(&Value::String("gpt-4o".into())),
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": body.get("max_tokens"),
        "temperature": body.get("temperature"),
        "stream": body.get("stream"),
    });

    chat_completions(State(state), Extension(auth), None, Json(chat_body)).await
}

/// `POST /v1/embeddings` — embeddings endpoint.
pub async fn embeddings(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let model = body.get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("text-embedding-3-small")
        .to_string();

    let provider = match state.llm_router.select(&model) {
        Ok(p) => p,
        Err(e) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "error": { "message": e.to_string(), "type": "model_not_found" }
            }))).into_response();
        }
    };

    let url = provider.embeddings_url();
    let mut req = reqwest::Client::new().post(&url);
    if let Some((header, value)) = provider.auth_header() {
        req = req.header(&header, &value);
    }
    req = req.header("Content-Type", "application/json").json(&body);

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let resp_body: Value = resp.json().await.unwrap_or(serde_json::json!({"error": "Invalid response"}));
            (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                Json(resp_body)).into_response()
        }
        Err(e) => {
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                "error": { "message": e.to_string(), "type": "provider_error" }
            }))).into_response()
        }
    }
}

/// `GET /v1/models` — list available models.
pub async fn list_models(
    State(state): State<Arc<AppState>>,
    Extension(_auth): Extension<RequireAuth>,
) -> impl IntoResponse {
    let models: Vec<Value> = state.llm_router.list_models()
        .into_iter()
        .filter(|m| m != "*")
        .map(|id| serde_json::json!({
            "id": id,
            "object": "model",
            "owned_by": "sentinel-gateway",
        }))
        .collect();

    (StatusCode::OK, Json(serde_json::json!({
        "object": "list",
        "data": models
    }))).into_response()
}

// ── Multimodal endpoints (OpenAI-compatible passthrough) ────────────────────

/// Helper: route an OpenAI-compatible multimodal request to the provider for a given model.
/// Audits and records usage like a 1-token request (since per-token pricing doesn't apply).
async fn passthrough_openai_endpoint(
    state: &Arc<AppState>,
    auth: &RequireAuth,
    model: &str,
    url_fn: impl Fn(&gateway_llm::LlmProvider) -> String,
    body: Value,
    event_name: &'static str,
) -> axum::response::Response {
    let provider = match state.llm_router.select(model) {
        Ok(p) => p,
        Err(e) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "error": { "message": e.to_string(), "type": "model_not_found" }
            }))).into_response();
        }
    };

    if !provider.supports_multimodal() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": {
                "message": format!("Provider {} does not support {event_name}", provider.provider_type),
                "type": "unsupported_endpoint"
            }
        }))).into_response();
    }

    let url = url_fn(&provider);
    let mut req = reqwest::Client::new().post(&url);
    if let Some((header, value)) = provider.auth_header() {
        req = req.header(&header, &value);
    }
    req = req.header("Content-Type", "application/json").json(&body);

    let started = Instant::now();
    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let resp_body: Value = resp.json().await.unwrap_or(serde_json::json!({"error": "Invalid response"}));

            // Best-effort usage record — multimodal often doesn't return tokens, so we
            // record 0 tokens and the actual latency for observability.
            let _ = state.usage_record_repo.create(
                gateway_db::models::usage_record::CreateUsageRecord {
                    tenant_id: auth.0.tenant_id,
                    user_id: Some(auth.0.user_id),
                    api_key_id: match &auth.0.method {
                        gateway_auth::AuthMethod::ApiKey { key_id, .. } => Some(*key_id),
                        _ => None,
                    },
                    backend_id: provider.id,
                    model: Some(model.to_string()),
                    tokens_input: 0,
                    tokens_output: 0,
                    cost_usd: 0.0,
                    latency_ms: started.elapsed().as_millis() as i64,
                    status_code: status.as_u16() as i32,
                    error: if status.is_success() { None } else { Some(event_name.to_string()) },
                }
            ).await;

            (StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                Json(resp_body)).into_response()
        }
        Err(e) => {
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                "error": { "message": e.to_string(), "type": "provider_error" }
            }))).into_response()
        }
    }
}

/// `POST /v1/images/generations` — OpenAI DALL-E / SDXL-compatible image generation.
pub async fn images_generations(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let model = body.get("model").and_then(|m| m.as_str()).unwrap_or("dall-e-3").to_string();
    passthrough_openai_endpoint(
        &state, &auth, &model,
        |p| p.images_generations_url(),
        body, "images.generations",
    ).await
}

/// `POST /v1/images/edits` — image edit (DALL-E 2).
pub async fn images_edits(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let model = body.get("model").and_then(|m| m.as_str()).unwrap_or("dall-e-2").to_string();
    passthrough_openai_endpoint(
        &state, &auth, &model,
        |p| p.images_edits_url(),
        body, "images.edits",
    ).await
}

/// `POST /v1/audio/transcriptions` — Whisper-compatible audio transcription.
pub async fn audio_transcriptions(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let model = body.get("model").and_then(|m| m.as_str()).unwrap_or("whisper-1").to_string();
    passthrough_openai_endpoint(
        &state, &auth, &model,
        |p| p.audio_transcriptions_url(),
        body, "audio.transcriptions",
    ).await
}

/// `POST /v1/audio/speech` — OpenAI TTS-compatible speech synthesis.
pub async fn audio_speech(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let model = body.get("model").and_then(|m| m.as_str()).unwrap_or("tts-1").to_string();
    passthrough_openai_endpoint(
        &state, &auth, &model,
        |p| p.audio_speech_url(),
        body, "audio.speech",
    ).await
}
