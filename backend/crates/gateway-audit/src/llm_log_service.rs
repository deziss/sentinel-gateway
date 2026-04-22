//! Async write-behind buffer for LLM logs.
//!
//! The LLM handler enqueues a `CreateLlmLog` for every request. This service
//! batches them into bulk INSERTs to amortise DB round-trips. Mirror of the
//! `AuditService` design.
//!
//! Flush errors increment the `llm_log_write_errors_total` Prometheus counter
//! so that silent data-loss is observable and alertable.

use gateway_db::{
    models::llm_log::CreateLlmLog,
    repository::LlmLogRepository,
};
use gateway_telemetry::metrics::REGISTRY;
use once_cell::sync::Lazy;
use prometheus::{CounterVec, Opts};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{self, Sender};
use tracing::{error, info};

// ── Per-module metric (avoids a full Metrics handle dependency) ────────────

static LLM_LOG_WRITE_ERRORS: Lazy<CounterVec> = Lazy::new(|| {
    let c = CounterVec::new(
        Opts::new(
            "llm_log_write_errors_total",
            "Total LLM log batch-write failures (fire-and-forget path)",
        ),
        &["error_kind"],
    )
    .expect("valid metric");
    // Register into the shared registry; ignore if already registered
    // (e.g., when gateway-telemetry::Metrics::new() ran first).
    REGISTRY.register(Box::new(c.clone())).ok();
    c
});

// ── Service ────────────────────────────────────────────────────────────────

/// Async LLM log writer — batches up to `batch_size` records or flushes on timeout.
#[derive(Clone)]
pub struct LlmLogService {
    sender: Sender<CreateLlmLog>,
}

impl LlmLogService {
    pub fn start(repo: Arc<LlmLogRepository>, batch_size: usize, flush_interval_ms: u64) -> Self {
        let (tx, mut rx) = mpsc::channel::<CreateLlmLog>(50_000);

        tokio::spawn(async move {
            let mut buffer: Vec<CreateLlmLog> = Vec::with_capacity(batch_size);
            let mut ticker = tokio::time::interval(Duration::from_millis(flush_interval_ms));

            loop {
                tokio::select! {
                    Some(entry) = rx.recv() => {
                        buffer.push(entry);
                        if buffer.len() >= batch_size {
                            flush(&repo, &mut buffer).await;
                        }
                    }
                    _ = ticker.tick() => {
                        if !buffer.is_empty() {
                            flush(&repo, &mut buffer).await;
                        }
                    }
                }
            }
        });

        Self { sender: tx }
    }

    /// Enqueue a log entry. Non-blocking; drops silently if buffer full
    /// (we never want LLM request path to block on logging).
    /// Channel-full drops are counted at DEBUG level — not fatal.
    pub fn log(&self, entry: CreateLlmLog) {
        if let Err(e) = self.sender.try_send(entry) {
            // Increment counter so drops are visible in dashboards.
            LLM_LOG_WRITE_ERRORS.with_label_values(&["channel_full"]).inc();
            tracing::debug!("LLM log buffer full, dropping: {e}");
        }
    }
}

async fn flush(repo: &LlmLogRepository, buffer: &mut Vec<CreateLlmLog>) {
    let batch: Vec<CreateLlmLog> = std::mem::take(buffer);
    let count = batch.len();
    if let Err(e) = repo.batch_insert(batch).await {
        // Classify the error kind for finer-grained alerting.
        let kind = if e.to_string().contains("timeout") { "db_timeout" } else { "db_error" };
        LLM_LOG_WRITE_ERRORS.with_label_values(&[kind]).inc();
        error!(
            error = %e,
            count,
            "LLM log batch insert FAILED — {count} records lost. \
             Check llm_log_write_errors_total counter and set up an alert."
        );
    } else {
        info!(count, "LLM log batch flushed");
    }
}
