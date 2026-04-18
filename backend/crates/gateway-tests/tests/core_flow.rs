//! Integration tests for gateway-core: proxy utilities, path rewriting,
//! GraphQL depth, IP extraction, circuit breaker.

use axum::http::HeaderMap;
use gateway_core::{
    circuit_breaker::CircuitBreaker,
    graphql,
    grpc,
    ip,
    rewrite,
    websocket,
};
use serde_json::json;
use uuid::Uuid;

// ── IP Extraction ─────────────────────────────────────────────────────────

#[test]
fn ip_extract_prefers_x_real_ip() {
    let mut h = HeaderMap::new();
    h.insert("x-real-ip", "203.0.113.5".parse().unwrap());
    h.insert("x-forwarded-for", "1.2.3.4".parse().unwrap());

    assert_eq!(ip::extract_client_ip(&h, None), "203.0.113.5");
}

#[test]
fn ip_extract_falls_through_xff_first_ip() {
    let mut h = HeaderMap::new();
    h.insert("x-forwarded-for", "1.2.3.4, 10.0.0.1, 192.168.0.1".parse().unwrap());

    assert_eq!(ip::extract_client_ip(&h, None), "1.2.3.4");
}

#[test]
fn ip_extract_cloudflare_header() {
    let mut h = HeaderMap::new();
    h.insert("cf-connecting-ip", "198.51.100.42".parse().unwrap());

    assert_eq!(ip::extract_client_ip(&h, None), "198.51.100.42");
}

#[test]
fn ip_extract_returns_unknown_when_missing() {
    let h = HeaderMap::new();
    assert_eq!(ip::extract_client_ip(&h, None), "unknown");
}

// ── Path Rewriting ────────────────────────────────────────────────────────

#[test]
fn rewrite_strip_prefix_simple() {
    assert_eq!(
        rewrite::rewrite_path("/api/v1/users", "/api/v1", true, &json!({})),
        "/users"
    );
}

#[test]
fn rewrite_strip_prefix_exact() {
    assert_eq!(
        rewrite::rewrite_path("/api/v1", "/api/v1", true, &json!({})),
        "/"
    );
}

#[test]
fn rewrite_regex_rules() {
    let rules = json!({"/old/(.*)": "/new/$1"});
    assert_eq!(
        rewrite::rewrite_path("/old/users/42", "", false, &rules),
        "/new/users/42"
    );
}

#[test]
fn rewrite_no_strip_no_rules() {
    assert_eq!(
        rewrite::rewrite_path("/untouched/path", "/api", false, &json!({})),
        "/untouched/path"
    );
}

// ── GraphQL Depth ─────────────────────────────────────────────────────────

#[test]
fn graphql_depth_simple() {
    assert_eq!(graphql::query_depth("{ users { name } }"), 2);
}

#[test]
fn graphql_depth_nested() {
    assert_eq!(
        graphql::query_depth("{ a { b { c { d { e } } } } }"),
        5
    );
}

#[test]
fn graphql_introspection_detection() {
    assert!(graphql::is_introspection("{ __schema { types { name } } }"));
    assert!(graphql::is_introspection("{ __type(name: \"User\") { fields { name } } }"));
    assert!(!graphql::is_introspection("{ users { name } }"));
}

#[test]
fn graphql_extract_query_from_body() {
    let body = json!({"query": "{ users { id } }", "variables": {}});
    assert_eq!(graphql::extract_query(&body), Some("{ users { id } }"));
}

// ── gRPC Detection ────────────────────────────────────────────────────────

#[test]
fn grpc_detects_content_type() {
    let mut h = HeaderMap::new();
    h.insert("content-type", "application/grpc".parse().unwrap());
    assert!(grpc::is_grpc(&h));

    h.insert("content-type", "application/grpc+proto".parse().unwrap());
    assert!(grpc::is_grpc(&h));
}

#[test]
fn grpc_ignores_json() {
    let mut h = HeaderMap::new();
    h.insert("content-type", "application/json".parse().unwrap());
    assert!(!grpc::is_grpc(&h));
}

#[test]
fn grpc_prepare_headers_strips_hop_by_hop() {
    let mut h = HeaderMap::new();
    h.insert("host", "example.com".parse().unwrap());
    h.insert("connection", "upgrade".parse().unwrap());
    h.insert("grpc-timeout", "10S".parse().unwrap());
    h.insert("authorization", "Bearer x".parse().unwrap());

    let prepared = grpc::prepare_grpc_headers(&h);
    assert!(!prepared.contains_key("host"));
    assert!(!prepared.contains_key("connection"));
    assert!(prepared.contains_key("grpc-timeout"));
    assert!(prepared.contains_key("authorization"));
}

// ── WebSocket Detection ───────────────────────────────────────────────────

#[test]
fn websocket_detects_upgrade() {
    let mut h = HeaderMap::new();
    h.insert("upgrade", "websocket".parse().unwrap());
    h.insert("connection", "Upgrade".parse().unwrap());

    assert!(websocket::is_websocket_upgrade(&h));
}

#[test]
fn websocket_requires_both_headers() {
    let mut h = HeaderMap::new();
    h.insert("upgrade", "websocket".parse().unwrap());
    // Missing connection: upgrade
    assert!(!websocket::is_websocket_upgrade(&h));
}

#[test]
fn websocket_url_conversion() {
    assert_eq!(websocket::to_ws_url("http://api.example.com/socket"), "ws://api.example.com/socket");
    assert_eq!(websocket::to_ws_url("https://api.example.com/socket"), "wss://api.example.com/socket");
    assert_eq!(websocket::to_ws_url("ws://already.com"), "ws://already.com");
}

// ── Circuit Breaker ───────────────────────────────────────────────────────

#[test]
fn circuit_breaker_opens_after_threshold() {
    let cb = CircuitBreaker::new(3, 60);
    let backend = Uuid::new_v4();

    // Initially closed
    assert!(!cb.is_open(backend));

    // Record 3 failures → should open
    cb.record_failure(backend);
    cb.record_failure(backend);
    cb.record_failure(backend);
    assert!(cb.is_open(backend));
}

#[test]
fn circuit_breaker_closes_on_success() {
    let cb = CircuitBreaker::new(3, 60);
    let backend = Uuid::new_v4();

    cb.record_failure(backend);
    cb.record_failure(backend);
    cb.record_failure(backend);
    assert!(cb.is_open(backend));

    cb.record_success(backend);
    assert!(!cb.is_open(backend));
}

#[test]
fn circuit_breaker_isolates_backends() {
    let cb = CircuitBreaker::new(2, 60);
    let b1 = Uuid::new_v4();
    let b2 = Uuid::new_v4();

    cb.record_failure(b1);
    cb.record_failure(b1);
    assert!(cb.is_open(b1));
    assert!(!cb.is_open(b2));
}
