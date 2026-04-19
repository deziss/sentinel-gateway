//! Async write-behind buffer for LLM logs.
//!
//! The LLM handler enqueues a `CreateLlmLog` for every request. This service
//! batches them into bulk INSERTs to amortize DB round-trips. Mirror of the
//! `AuditService` design.

use gateway_db::{
    models::llm_log::CreateLlmLog,
    repository::LlmLogRepository,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{self, Sender};
use tracing::{error, info};

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
    pub fn log(&self, entry: CreateLlmLog) {
        if let Err(e) = self.sender.try_send(entry) {
            // Log at DEBUG — loud WARN on every drop would drown the logs
            tracing::debug!("LLM log buffer full, dropping: {e}");
        }
    }
}

async fn flush(repo: &LlmLogRepository, buffer: &mut Vec<CreateLlmLog>) {
    let batch: Vec<CreateLlmLog> = buffer.drain(..).collect();
    let count = batch.len();
    if let Err(e) = repo.batch_insert(batch).await {
        error!("LLM log batch insert failed ({count} records): {e}");
    } else {
        info!(count, "LLM log batch flushed");
    }
}
