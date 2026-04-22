use std::sync::Arc;
use tokio::sync::mpsc::{self, Sender};
use tracing::{error, info};

use gateway_db::{
    models::audit_log::CreateAuditLog,
    repository::{AuditLogRepository, WebhookEndpointRepository},
};

use crate::events::AuditEvent;
use crate::webhook::WebhookDispatcher;

/// Async buffered audit writer with webhook integration.
/// Events are enqueued and batch-written to PostgreSQL.
/// Webhook notifications are dispatched in parallel.
pub struct AuditService {
    sender: Sender<AuditEvent>,
}

impl AuditService {
    /// Start the background flush loop and return an AuditService handle.
    pub fn start(repo: Arc<AuditLogRepository>, batch_size: usize, flush_interval_ms: u64) -> Self {
        let (tx, mut rx) = mpsc::channel::<AuditEvent>(10_000);

        tokio::spawn(async move {
            let mut buffer: Vec<AuditEvent> = Vec::with_capacity(batch_size);
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_millis(flush_interval_ms));

            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        buffer.push(event);
                        if buffer.len() >= batch_size {
                            flush(&repo, &mut buffer).await;
                        }
                    }
                    _ = interval.tick() => {
                        if !buffer.is_empty() {
                            flush(&repo, &mut buffer).await;
                        }
                    }
                }
            }
        });

        Self { sender: tx }
    }

    /// Start the background flush loop with webhook dispatching.
    pub fn start_with_webhooks(
        repo: Arc<AuditLogRepository>,
        webhook_repo: Arc<WebhookEndpointRepository>,
        batch_size: usize,
        flush_interval_ms: u64,
        webhook_max_retries: u32,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<AuditEvent>(10_000);
        let dispatcher = Arc::new(WebhookDispatcher::new(webhook_max_retries));

        tokio::spawn(async move {
            let mut buffer: Vec<AuditEvent> = Vec::with_capacity(batch_size);
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_millis(flush_interval_ms));

            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        // Dispatch webhook for each event immediately (non-blocking)
                        dispatch_webhook(&dispatcher, &webhook_repo, &event).await;
                        buffer.push(event);
                        if buffer.len() >= batch_size {
                            flush(&repo, &mut buffer).await;
                        }
                    }
                    _ = interval.tick() => {
                        if !buffer.is_empty() {
                            flush(&repo, &mut buffer).await;
                        }
                    }
                }
            }
        });

        Self { sender: tx }
    }

    /// Enqueue an audit event (non-blocking, fire-and-forget).
    pub fn log(&self, event: AuditEvent) {
        if let Err(e) = self.sender.try_send(event) {
            error!("Audit buffer full, dropping event: {e}");
        }
    }
}

async fn flush(repo: &AuditLogRepository, buffer: &mut Vec<AuditEvent>) {
    let logs: Vec<CreateAuditLog> = buffer
        .drain(..)
        .map(|e| CreateAuditLog {
            tenant_id: e.tenant_id,
            user_id: e.user_id,
            action: e.event_type.to_string(),
            resource_type: e.resource_type,
            resource_id: e.resource_id,
            details: e.details,
            ip_address: e.ip_address,
            user_agent: e.user_agent,
        })
        .collect();

    let count = logs.len();
    if let Err(e) = repo.batch_insert(logs).await {
        error!("Audit batch insert failed: {e}");
    } else {
        info!(count, "Audit batch flushed");
    }
}

async fn dispatch_webhook(
    dispatcher: &WebhookDispatcher,
    webhook_repo: &WebhookEndpointRepository,
    event: &AuditEvent,
) {
    let endpoints = match webhook_repo.list_active_by_tenant(event.tenant_id).await {
        Ok(e) => e,
        Err(e) => {
            error!("Failed to load webhook endpoints: {e}");
            return;
        }
    };

    if endpoints.is_empty() {
        return;
    }

    // Dispatch in a separate task to not block the audit pipeline
    let dispatcher = dispatcher.clone();
    let event = event.clone();
    tokio::spawn(async move {
        dispatcher.dispatch(&event, &endpoints).await;
    });
}
