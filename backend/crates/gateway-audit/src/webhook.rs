use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tracing::{error, warn};
use hex;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use gateway_db::models::WebhookEndpoint;
use crate::events::AuditEvent;

#[derive(Clone)]
pub struct WebhookDispatcher {
    client: Client,
    max_retries: u32,
}

impl WebhookDispatcher {
    pub fn new(max_retries: u32) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            max_retries,
        }
    }

    /// Dispatch an event to all matching webhook endpoints.
    pub async fn dispatch(&self, event: &AuditEvent, endpoints: &[WebhookEndpoint]) {
        let event_type = event.event_type.to_string();
        let payload = json!({
            "event": event_type,
            "tenant_id": event.tenant_id,
            "timestamp": event.timestamp,
            "data": event.details,
        });
        let payload_str = serde_json::to_string(&payload).unwrap_or_default();

        for endpoint in endpoints {
            if !endpoint.events.contains(&event_type) && !endpoint.events.contains(&"*".to_string()) {
                continue;
            }
            let sig = self.hmac_signature(&endpoint.secret, &payload_str);
            self.send_with_retry(&endpoint.url, &payload_str, &sig).await;
        }
    }

    async fn send_with_retry(&self, url: &str, payload: &str, signature: &str) {
        let mut attempt = 0u32;
        loop {
            match self
                .client
                .post(url)
                .header("Content-Type", "application/json")
                .header("X-Sentinel-Signature", signature)
                .body(payload.to_owned())
                .send()
                .await
            {
                Ok(r) if r.status().is_success() => return,
                Ok(r) => {
                    warn!("Webhook {url} returned {}: attempt {attempt}", r.status());
                }
                Err(e) => {
                    warn!("Webhook {url} error: {e}: attempt {attempt}");
                }
            }
            attempt += 1;
            if attempt >= self.max_retries {
                error!("Webhook {url} failed after {attempt} attempts");
                return;
            }
            let delay = Duration::from_millis(500 * 2u64.pow(attempt));
            tokio::time::sleep(delay).await;
        }
    }

    fn hmac_signature(&self, secret: &str, payload: &str) -> String {
        type HmacSha256 = Hmac<Sha256>;
        
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("HMAC can take any key size");
        mac.update(payload.as_bytes());
        
        let result = mac.finalize();
        format!("sha256={}", hex::encode(result.into_bytes()))
    }
}
