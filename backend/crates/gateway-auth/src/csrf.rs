//! CSRF protection middleware using the double-submit cookie pattern.
//!
//! This middleware is intended for cookie-based web UI sessions. It is **not**
//! wired into the route stack by default because the gateway currently uses
//! header-based auth (JWT / API key). Apply it to web UI routes when
//! cookie-based sessions are introduced.

use axum::{
    extract::Request,
    http::{HeaderMap, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;

const CSRF_COOKIE_NAME: &str = "sg_csrf";
const CSRF_HEADER_NAME: &str = "X-CSRF-Token";

/// Generate a cryptographically random CSRF token (32 bytes, base64-url).
pub fn generate_csrf_token() -> String {
    let mut rng = rand::thread_rng();
    let raw: Vec<u8> = (0..32).map(|_| rng.gen::<u8>()).collect();
    URL_SAFE_NO_PAD.encode(&raw)
}

/// CSRF middleware using the double-submit cookie pattern.
///
/// - Skips safe methods (GET, HEAD, OPTIONS).
/// - Skips requests that use header-based auth (`Authorization` or `X-API-Key`),
///   since those are not vulnerable to CSRF.
/// - For cookie-authenticated state-changing requests, validates that the
///   `X-CSRF-Token` header matches the `sg_csrf` cookie value.
pub async fn csrf_middleware(req: Request, next: Next) -> Response {
    // Safe methods don't need CSRF protection
    if matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        return next.run(req).await;
    }

    // Skip if using header-based auth (not vulnerable to CSRF)
    if req.headers().contains_key("Authorization") || req.headers().contains_key("X-API-Key") {
        return next.run(req).await;
    }

    // For cookie-based auth: validate double-submit pattern
    let cookie_token = extract_csrf_cookie(req.headers());
    let header_token = req.headers()
        .get(CSRF_HEADER_NAME)
        .and_then(|v| v.to_str().ok());

    match (cookie_token.as_deref(), header_token) {
        (Some(c), Some(h)) if !c.is_empty() && c == h => next.run(req).await,
        _ => (StatusCode::FORBIDDEN, "CSRF validation failed").into_response(),
    }
}

fn extract_csrf_cookie(headers: &HeaderMap) -> Option<String> {
    headers.get("Cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix(&format!("{CSRF_COOKIE_NAME}="))
                    .map(|v| v.to_string())
            })
        })
}
