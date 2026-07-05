use axum::{Json, extract::State, http::{HeaderMap, StatusCode}, response::IntoResponse, Extension};
use axum::extract::ConnectInfo;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use validator::Validate;

use crate::state::AppState;
use gateway_auth::middleware::{RequireAuth, extract_bearer_token};
use gateway_auth::password::PasswordService;
use gateway_auth::roles::Role;
use gateway_audit::events::{AuditEvent, EventType};
use gateway_db::models::user::{UserRole, UserStatus};

fn extract_client_ip(headers: &HeaderMap, addr: Option<&ConnectInfo<SocketAddr>>) -> String {
    headers.get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| headers.get("x-real-ip").and_then(|v| v.to_str().ok()).map(|s| s.to_string()))
        .unwrap_or_else(|| addr.map(|a| a.0.ip().to_string()).unwrap_or_else(|| "unknown".to_string()))
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(length(min = 1, max = 255))]
    pub tenant_slug: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1, max = 255))]
    pub password: String,
    pub mfa_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    // Validate input
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let client_ip = extract_client_ip(&headers, connect_info.as_ref());

    // 0. Per-IP rate limit on login (defense against brute-force + enumeration)
    // 10 req/min per IP — tight enough to prevent abuse, loose enough for legit retries
    let ip_key = gateway_policy::RateLimitKey::Ip(format!("login:{client_ip}"));
    if state.policy_engine.rate_limiter.check(&ip_key, 10).await.is_err() {
        state.metrics.record_rate_limited("unknown", "login_ip");
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [
                ("retry-after", "60"),
                ("x-ratelimit-limit", "10"),
                ("x-ratelimit-remaining", "0"),
            ],
            Json(serde_json::json!({"error": "Too many login attempts. Please try again later."})),
        ).into_response();
    }

    // 1. Resolve tenant
    let tenant = match state.tenant_repo.find_by_slug(&body.tenant_slug).await {
        Ok(t) => t,
        Err(_) => {
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Invalid credentials"}))).into_response();
        }
    };

    // 2. Find user
    let user = match state.user_repo.find_by_email(&body.email, tenant.id).await {
        Ok(u) => u,
        Err(_) => {
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Invalid credentials"}))).into_response();
        }
    };

    // 3. Check account lockout
    if user.status == UserStatus::Locked {
        if let Some(until) = user.locked_until {
            if until > Utc::now() {
                return (StatusCode::LOCKED, Json(serde_json::json!({
                    "error": "Account temporarily locked. Try again later."
                }))).into_response();
            }
        }
    }

    // 4. Verify password
    let password_ok = match PasswordService::verify(&body.password, &user.password_hash) {
        Ok(ok) => ok,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Internal error"}))).into_response();
        }
    };

    if !password_ok {
        let attempts = state.user_repo.increment_failed_attempts(user.id).await.unwrap_or(0);

        if attempts >= state.auth_config.max_failed_logins {
            let _ = state.user_repo.lock_user(user.id, state.auth_config.lockout_duration_minutes).await;
        }

        state.audit_service.log(
            AuditEvent::new(tenant.id, EventType::UserLoginFailed, "auth")
                .with_user(user.id)
                .with_details(serde_json::json!({"attempts": attempts}))
                .with_ip(&client_ip)
        );

        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Invalid credentials"}))).into_response();
    }

    // 4.5. Verify MFA if enabled
    if let Some(mfa_secret) = &user.mfa_secret {
        let provided_token = match &body.mfa_token {
            Some(t) => t,
            None => {
                return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "MFA token required"}))).into_response();
            }
        };

        let secret = totp_rs::Secret::Encoded(mfa_secret.to_string());
        let totp = match totp_rs::TOTP::new(
            totp_rs::Algorithm::SHA1,
            6,
            1,
            30,
            secret.to_bytes().unwrap_or_default(),
        ) {
            Ok(totp) => totp,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Invalid MFA configuration"}))).into_response(),
        };

        if !totp.check_current(provided_token).unwrap_or(false) {
            return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Invalid MFA token"}))).into_response();
        }
    }

    // 5. Success — clear lockout
    let _ = state.user_repo.reset_failed_attempts(user.id).await;

    // 6. Map DB role -> auth role
    let role = match user.role {
        UserRole::SuperAdmin => Role::SuperAdmin,
        UserRole::TenantAdmin => Role::TenantAdmin,
        UserRole::User => Role::User,
        UserRole::ReadOnly => Role::ReadOnly,
    };

    // 7. Issue tokens
    let access_token = match state.jwt.issue_access_token(user.id, tenant.id, role.clone()) {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to generate token"}))).into_response(),
    };

    let refresh_token = match state.jwt.issue_refresh_token(user.id, tenant.id, role) {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to generate token"}))).into_response(),
    };

    // 8. Audit
    state.audit_service.log(
        AuditEvent::new(tenant.id, EventType::UserLogin, "auth")
            .with_user(user.id)
            .with_resource_id(user.id.to_string())
            .with_details(serde_json::json!({"method": "password"}))
            .with_ip(&client_ip)
    );

    let resp = LoginResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: state.auth_config.access_token_ttl_minutes * 60,
    };

    (StatusCode::OK, Json(resp)).into_response()
}

// ─── Token Refresh ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Validate)]
pub struct RefreshRequest {
    #[validate(length(min = 1))]
    pub refresh_token: String,
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RefreshRequest>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e.to_string()}))).into_response();
    }

    let claims = match state.jwt.validate(&body.refresh_token) {
        Ok(c) => c,
        Err(e) => {
            let code = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::UNAUTHORIZED);
            return (code, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    if claims.typ != "refresh" {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Not a refresh token"}))).into_response();
    }

    if state.token_blacklist.is_revoked(&claims.jti) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "Token has been revoked"}))).into_response();
    }

    let remaining_secs = (claims.exp - Utc::now().timestamp()).max(0) as u64;
    state.token_blacklist.revoke(&claims.jti, Duration::from_secs(remaining_secs));

    let user_id = claims.user_id();
    let tenant_id = claims.tenant_id();
    let role = claims.role.clone();

    let access_token = match state.jwt.issue_access_token(user_id, tenant_id, role.clone()) {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to generate token"}))).into_response(),
    };

    let refresh_token = match state.jwt.issue_refresh_token(user_id, tenant_id, role) {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "Failed to generate token"}))).into_response(),
    };

    let resp = LoginResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: state.auth_config.access_token_ttl_minutes * 60,
    };

    (StatusCode::OK, Json(resp)).into_response()
}

// ─── Logout ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: Option<String>,
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<RequireAuth>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    body: Option<Json<LogoutRequest>>,
) -> impl IntoResponse {
    let client_ip = extract_client_ip(&headers, connect_info.as_ref());

    if let Some(token) = extract_bearer_token(&headers) {
        if let Ok(claims) = state.jwt.validate(token) {
            let remaining = (claims.exp - Utc::now().timestamp()).max(0) as u64;
            state.token_blacklist.revoke(&claims.jti, Duration::from_secs(remaining));
        }
    }

    if let Some(Json(req)) = body {
        if let Some(ref rt) = req.refresh_token {
            if let Ok(claims) = state.jwt.validate(rt) {
                let remaining = (claims.exp - Utc::now().timestamp()).max(0) as u64;
                state.token_blacklist.revoke(&claims.jti, Duration::from_secs(remaining));
            }
        }
    }

    state.audit_service.log(
        AuditEvent::new(auth.0.tenant_id, EventType::UserLogout, "auth")
            .with_user(auth.0.user_id)
            .with_ip(&client_ip)
    );

    (StatusCode::OK, Json(serde_json::json!({"message": "Logged out successfully"})))
}
