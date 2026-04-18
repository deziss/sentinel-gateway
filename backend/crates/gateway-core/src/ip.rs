use axum::http::HeaderMap;
use std::net::SocketAddr;

/// Extract the real client IP address from request headers.
///
/// Checks in priority order:
/// 1. `X-Real-IP` header (set by reverse proxies like Nginx)
/// 2. `X-Forwarded-For` header (first IP in the chain)
/// 3. `CF-Connecting-IP` (Cloudflare)
/// 4. `True-Client-IP` (Akamai/Cloudflare Enterprise)
/// 5. Socket address from `ConnectInfo`
/// 6. Fallback to "unknown"
pub fn extract_client_ip(headers: &HeaderMap, connect_info: Option<&SocketAddr>) -> String {
    // 1. X-Real-IP
    if let Some(ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let ip = ip.trim();
        if !ip.is_empty() {
            return ip.to_string();
        }
    }

    // 2. X-Forwarded-For (first IP)
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = xff.split(',').next() {
            let ip = first.trim();
            if !ip.is_empty() {
                return ip.to_string();
            }
        }
    }

    // 3. CF-Connecting-IP (Cloudflare)
    if let Some(ip) = headers.get("cf-connecting-ip").and_then(|v| v.to_str().ok()) {
        let ip = ip.trim();
        if !ip.is_empty() {
            return ip.to_string();
        }
    }

    // 4. True-Client-IP
    if let Some(ip) = headers.get("true-client-ip").and_then(|v| v.to_str().ok()) {
        let ip = ip.trim();
        if !ip.is_empty() {
            return ip.to_string();
        }
    }

    // 5. Socket address
    if let Some(addr) = connect_info {
        return addr.ip().to_string();
    }

    "unknown".to_string()
}
