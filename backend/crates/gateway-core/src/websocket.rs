use axum::extract::ws::{Message as AxumMsg, WebSocket};
use axum::http::HeaderMap;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message as TungMsg};
use tracing::debug;

use crate::error::CoreError;

/// Detect if a request is a WebSocket upgrade request.
pub fn is_websocket_upgrade(headers: &HeaderMap) -> bool {
    let upgrade = headers.get("upgrade")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase());

    let connection = headers.get("connection")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase());

    matches!(
        (upgrade.as_deref(), connection.as_deref()),
        (Some("websocket"), Some(conn)) if conn.contains("upgrade")
    )
}

/// Build the upstream WebSocket URL from the HTTP target URL.
pub fn to_ws_url(http_url: &str) -> String {
    if http_url.starts_with("https://") {
        format!("wss://{}", &http_url[8..])
    } else if http_url.starts_with("http://") {
        format!("ws://{}", &http_url[7..])
    } else if http_url.starts_with("wss://") || http_url.starts_with("ws://") {
        http_url.to_string()
    } else {
        format!("ws://{http_url}")
    }
}

/// Proxy a WebSocket connection bidirectionally between client and upstream.
pub async fn relay_websocket(
    client_ws: WebSocket,
    upstream_url: &str,
) -> Result<(), CoreError> {
    let ws_url = to_ws_url(upstream_url);
    debug!("Connecting to upstream WebSocket: {ws_url}");

    let (upstream_ws, _) = connect_async(&ws_url)
        .await
        .map_err(|e| CoreError::WebSocket(format!("Failed to connect to upstream: {e}")))?;

    let (mut client_sink, mut client_stream) = client_ws.split();
    let (mut upstream_sink, mut upstream_stream) = upstream_ws.split();

    // Client → Upstream relay
    let c2u = async {
        while let Some(Ok(msg)) = client_stream.next().await {
            let tung_msg = match msg {
                AxumMsg::Text(t) => TungMsg::Text(t.to_string()),
                AxumMsg::Binary(b) => TungMsg::Binary(b.to_vec()),
                AxumMsg::Ping(p) => TungMsg::Ping(p.to_vec()),
                AxumMsg::Pong(p) => TungMsg::Pong(p.to_vec()),
                AxumMsg::Close(_) => return,
            };
            if upstream_sink.send(tung_msg).await.is_err() {
                return;
            }
        }
    };

    // Upstream → Client relay
    let u2c = async {
        while let Some(Ok(msg)) = upstream_stream.next().await {
            let axum_msg = match msg {
                TungMsg::Text(t) => AxumMsg::Text(t.into()),
                TungMsg::Binary(b) => AxumMsg::Binary(b.into()),
                TungMsg::Ping(p) => AxumMsg::Ping(p.into()),
                TungMsg::Pong(p) => AxumMsg::Pong(p.into()),
                TungMsg::Close(_) => return,
                _ => continue,
            };
            if client_sink.send(axum_msg).await.is_err() {
                return;
            }
        }
    };

    tokio::select! {
        _ = c2u => debug!("Client→upstream relay ended"),
        _ = u2c => debug!("Upstream→client relay ended"),
    }

    Ok(())
}
