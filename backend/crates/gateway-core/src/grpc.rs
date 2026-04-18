use axum::http::HeaderMap;

/// Detect if a request is a gRPC request.
///
/// gRPC requests use HTTP/2 with `content-type: application/grpc` (or variants).
pub fn is_grpc(headers: &HeaderMap) -> bool {
    headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("application/grpc"))
        .unwrap_or(false)
}

/// Build the headers to forward for a gRPC request.
/// Preserves gRPC-specific headers while stripping hop-by-hop headers.
pub fn prepare_grpc_headers(original: &HeaderMap) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();

    for (name, value) in original {
        let name_str = name.as_str();
        // Forward all headers except hop-by-hop and auth (handled separately)
        match name_str {
            "host" | "connection" | "keep-alive" | "proxy-authenticate"
            | "proxy-authorization" | "te" | "trailer" | "transfer-encoding" => continue,
            _ => {
                headers.insert(name.clone(), value.clone());
            }
        }
    }

    headers
}
