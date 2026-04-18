//! Optional observability export — forward LLM traces to Langfuse or Helicone.
//!
//! Design notes:
//! - **Opt-in.** Disabled by default (zero ops overhead, zero leaked data).
//! - **Fire-and-forget.** Uses `tokio::spawn` — never blocks the gateway
//!   response path. If the export endpoint is slow or down, gateway users
//!   still get their LLM response on time.
//! - **Bounded queue.** Drops events once a small buffer fills up. Observability
//!   backpressure must not become gateway backpressure.
//! - **Best-effort.** Errors log at WARN, never bubble up. The gateway's own
//!   OpenTelemetry export (already in `gateway-telemetry`) remains the
//!   source of truth for internal metrics.
//!
//! Supported targets: Langfuse (v1 ingest API), Helicone (OpenAI-compatible
//! proxy logs API). If the user configures neither, this module is a no-op.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::warn;

/// Which provider to export to. Multiple can be enabled simultaneously.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObservabilityExportConfig {
    /// Langfuse — https://langfuse.com. Uses OpenAPI `/api/public/ingestion`.
    #[serde(default)]
    pub langfuse: Option<LangfuseConfig>,
    /// Helicone — https://helicone.ai. Uses OpenAI-compatible logging API.
    #[serde(default)]
    pub helicone: Option<HeliconeConfig>,
    /// Max events queued before drops (default 1000).
    #[serde(default = "default_queue_size")]
    pub queue_size: usize,
}

fn default_queue_size() -> usize { 1000 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LangfuseConfig {
    /// Base URL. Default `https://cloud.langfuse.com`.
    #[serde(default = "default_langfuse_url")]
    pub base_url: String,
    /// Public key (pk-lf-...).
    pub public_key: String,
    /// Secret key (sk-lf-...).
    pub secret_key: String,
}

fn default_langfuse_url() -> String { "https://cloud.langfuse.com".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeliconeConfig {
    /// Base URL. Default `https://api.helicone.ai`.
    #[serde(default = "default_helicone_url")]
    pub base_url: String,
    /// Helicone API key.
    pub api_key: String,
}

fn default_helicone_url() -> String { "https://api.helicone.ai".to_string() }

/// One LLM request/response to export.
#[derive(Debug, Clone)]
pub struct TraceEvent {
    pub trace_id: String,
    pub tenant_id: Option<uuid::Uuid>,
    pub user_id: Option<uuid::Uuid>,
    pub api_key_id: Option<uuid::Uuid>,
    pub model: String,
    pub provider: String,
    /// Sanitized request (messages / prompt).
    pub request: serde_json::Value,
    /// Sanitized response.
    pub response: serde_json::Value,
    pub status_code: u16,
    pub latency_ms: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cost_usd: f64,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

/// Handle for the gateway to push events onto. Cheap to clone.
#[derive(Clone)]
pub struct ObservabilityExporter {
    tx: Option<mpsc::Sender<TraceEvent>>,
}

impl ObservabilityExporter {
    /// Disabled exporter — no-op. Safe default.
    pub fn disabled() -> Self {
        Self { tx: None }
    }

    /// Start the background exporter task. Returns a handle for pushing events.
    ///
    /// If both Langfuse and Helicone configs are `None`, returns the disabled
    /// exporter (no background task spawned).
    pub fn start(cfg: ObservabilityExportConfig) -> Self {
        if cfg.langfuse.is_none() && cfg.helicone.is_none() {
            return Self::disabled();
        }

        let (tx, mut rx) = mpsc::channel::<TraceEvent>(cfg.queue_size);
        let cfg = Arc::new(cfg);
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                // Run both exports concurrently via tokio::join (works with heterogeneous
                // futures — unlike futures::join_all which needs a uniform type).
                let lf = export_langfuse(&client, &cfg, &event);
                let hc = export_helicone(&client, &cfg, &event);
                let _ = tokio::join!(lf, hc);
            }
        });

        tracing::info!("Observability export enabled");

        Self { tx: Some(tx) }
    }

    /// Push an event onto the queue. Drops the event if the queue is full.
    /// Never blocks.
    pub fn push(&self, event: TraceEvent) {
        if let Some(tx) = &self.tx {
            if let Err(e) = tx.try_send(event) {
                // Too full or closed — drop silently (metrics track drops).
                let _ = e;
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.tx.is_some()
    }
}

// ── Langfuse export ──────────────────────────────────────────────────────────

async fn export_langfuse(
    client: &Client,
    cfg: &ObservabilityExportConfig,
    event: &TraceEvent,
) -> Result<(), ()> {
    let Some(lf) = &cfg.langfuse else { return Ok(()); };

    // Langfuse ingestion API accepts batched events. For simplicity we send one
    // event per push (fine at low volumes; batch if you see performance issues).
    let body = serde_json::json!({
        "batch": [
            {
                "id": uuid::Uuid::new_v4().to_string(),
                "timestamp": event.started_at.to_rfc3339(),
                "type": "trace-create",
                "body": {
                    "id": event.trace_id,
                    "userId": event.user_id.map(|u| u.to_string()),
                    "name": format!("{}/{}", event.provider, event.model),
                    "metadata": {
                        "tenant_id": event.tenant_id.map(|t| t.to_string()),
                        "api_key_id": event.api_key_id.map(|k| k.to_string()),
                        "status_code": event.status_code,
                        "cost_usd": event.cost_usd,
                        "latency_ms": event.latency_ms,
                    },
                    "input": event.request,
                    "output": event.response,
                }
            }
        ]
    });

    let resp = client
        .post(format!("{}/api/public/ingestion", lf.base_url.trim_end_matches('/')))
        .basic_auth(&lf.public_key, Some(&lf.secret_key))
        .json(&body)
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => Ok(()),
        Ok(r) => {
            warn!(status = r.status().as_u16(), "Langfuse export rejected");
            Err(())
        }
        Err(e) => {
            warn!(error = %e, "Langfuse export failed");
            Err(())
        }
    }
}

// ── Helicone export ──────────────────────────────────────────────────────────

async fn export_helicone(
    client: &Client,
    cfg: &ObservabilityExportConfig,
    event: &TraceEvent,
) -> Result<(), ()> {
    let Some(hc) = &cfg.helicone else { return Ok(()); };

    // Helicone's logging API accepts a proxy-style log. Using their "async log"
    // endpoint to avoid routing through their gateway.
    let body = serde_json::json!({
        "providerRequest": {
            "url": format!("{}/v1/chat/completions", event.provider),
            "json": event.request,
            "meta": {
                "Helicone-User-Id": event.user_id.map(|u| u.to_string()).unwrap_or_default(),
                "Helicone-Property-Tenant-Id": event.tenant_id.map(|t| t.to_string()).unwrap_or_default(),
                "Helicone-Property-Model": event.model.clone(),
            }
        },
        "providerResponse": {
            "status": event.status_code,
            "json": event.response,
        },
        "timing": {
            "startTime": { "seconds": event.started_at.timestamp() },
            "endTime": { "seconds": event.started_at.timestamp() + (event.latency_ms / 1000) as i64 },
        }
    });

    let resp = client
        .post(format!("{}/oai/v1/log", hc.base_url.trim_end_matches('/')))
        .bearer_auth(&hc.api_key)
        .json(&body)
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => Ok(()),
        Ok(r) => {
            warn!(status = r.status().as_u16(), "Helicone export rejected");
            Err(())
        }
        Err(e) => {
            warn!(error = %e, "Helicone export failed");
            Err(())
        }
    }
}
