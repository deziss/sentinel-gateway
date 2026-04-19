//! Data-lake export: spool LLM logs as newline-delimited JSON (NDJSON) files
//! to a local directory, S3, or GCS. Customers run their own downstream ETL.
//!
//! Why NDJSON files (instead of streaming straight to warehouse)?
//!   - Format-agnostic: Parquet, DuckDB, BigQuery, Snowflake all ingest NDJSON.
//!   - No vendor lock-in: nothing runs in-process beyond file rotation.
//!   - Buffered: one file-rotation per period (default 5 min) bounds IO.
//!
//! Backends:
//!   - `file` (default)        — write to `dir/YYYY-MM-DD/tenant/HH-mm-ss.ndjson`
//!   - `s3://bucket/prefix/`   — PUT each rotated file (requires AWS creds via env)
//!   - `gs://bucket/prefix/`   — PUT each rotated file (requires GCS creds via env)
//!
//! For S3/GCS, we shell out to `aws s3 cp` / `gsutil cp`. This keeps the binary
//! slim; customers who already run these tools in their Docker image get a
//! first-class path, and those who don't can stick with `file`.

use gateway_db::models::llm_log::CreateLlmLog;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::{self, Sender};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Default, Serialize)]
pub struct DataLakeConfig {
    /// `file:///path`, `s3://bucket/prefix/`, or `gs://bucket/prefix/`.
    /// When None, exporter is disabled.
    pub destination: Option<String>,
    /// How long to buffer before rotating (seconds). Default 300 (5 min).
    pub rotate_interval_secs: u64,
    /// Max in-memory queue depth before records are dropped.
    pub max_queue: usize,
}

impl DataLakeConfig {
    pub fn from_env() -> Self {
        Self {
            destination: std::env::var("DATA_LAKE_DESTINATION").ok(),
            rotate_interval_secs: std::env::var("DATA_LAKE_ROTATE_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(300),
            max_queue: 100_000,
        }
    }
}

/// Async data-lake exporter. Fire-and-forget; drops silently if queue is full.
#[derive(Clone)]
pub struct DataLakeExporter {
    sender: Option<Sender<CreateLlmLog>>,
}

impl DataLakeExporter {
    pub fn disabled() -> Self {
        Self { sender: None }
    }

    pub fn start(cfg: DataLakeConfig) -> Self {
        let Some(destination) = cfg.destination.clone() else {
            info!("data_lake: no destination configured, exporter disabled");
            return Self::disabled();
        };

        let (tx, mut rx) = mpsc::channel::<CreateLlmLog>(cfg.max_queue);
        let rotate = Duration::from_secs(cfg.rotate_interval_secs);

        tokio::spawn(async move {
            let mut buffer: Vec<CreateLlmLog> = Vec::with_capacity(1024);
            let mut ticker = tokio::time::interval(rotate);
            ticker.tick().await; // skip immediate first tick

            loop {
                tokio::select! {
                    maybe = rx.recv() => {
                        match maybe {
                            Some(entry) => buffer.push(entry),
                            None => {
                                if !buffer.is_empty() {
                                    let _ = flush_batch(&destination, std::mem::take(&mut buffer)).await;
                                }
                                break;
                            }
                        }
                    }
                    _ = ticker.tick() => {
                        if !buffer.is_empty() {
                            let batch = std::mem::take(&mut buffer);
                            let dest = destination.clone();
                            tokio::spawn(async move {
                                if let Err(e) = flush_batch(&dest, batch).await {
                                    error!("data_lake flush failed: {e}");
                                }
                            });
                        }
                    }
                }
            }
        });

        info!(destination = ?cfg.destination, "data_lake exporter started");
        Self { sender: Some(tx) }
    }

    pub fn push(&self, entry: CreateLlmLog) {
        if let Some(tx) = &self.sender {
            if tx.try_send(entry).is_err() {
                debug!("data_lake queue full; dropping log");
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.sender.is_some()
    }
}

async fn flush_batch(destination: &str, batch: Vec<CreateLlmLog>) -> anyhow::Result<()> {
    if batch.is_empty() {
        return Ok(());
    }

    // Build NDJSON in memory
    let mut body = String::with_capacity(batch.len() * 512);
    for entry in &batch {
        match serde_json::to_string(entry) {
            Ok(line) => {
                body.push_str(&line);
                body.push('\n');
            }
            Err(e) => warn!("data_lake: skipping unserializable entry: {e}"),
        }
    }

    let now = chrono::Utc::now();
    let date = now.format("%Y-%m-%d");
    let time = now.format("%H-%M-%S");
    let filename = format!("llm_logs-{time}-{}.ndjson", uuid::Uuid::new_v4().simple());

    if let Some(path) = destination.strip_prefix("file://") {
        let mut out = PathBuf::from(path);
        out.push(date.to_string());
        tokio::fs::create_dir_all(&out).await?;
        out.push(&filename);
        let mut f = tokio::fs::File::create(&out).await?;
        f.write_all(body.as_bytes()).await?;
        f.flush().await?;
        debug!(path = %out.display(), records = batch.len(), "data_lake wrote file");
        return Ok(());
    }

    if destination.starts_with("s3://") || destination.starts_with("gs://") {
        // Write to tmp then shell out to aws/gsutil.
        let tmp = std::env::temp_dir().join(&filename);
        tokio::fs::write(&tmp, &body).await?;
        let key = format!(
            "{}/{date}/{filename}",
            destination.trim_end_matches('/')
        );
        let (cmd, args) = if destination.starts_with("s3://") {
            ("aws", vec!["s3".to_string(), "cp".to_string(), tmp.display().to_string(), key.clone()])
        } else {
            ("gsutil", vec!["cp".to_string(), tmp.display().to_string(), key.clone()])
        };
        let output = tokio::process::Command::new(cmd)
            .args(&args)
            .output()
            .await;
        let _ = tokio::fs::remove_file(&tmp).await;
        match output {
            Ok(o) if o.status.success() => {
                debug!(key, records = batch.len(), "data_lake uploaded");
            }
            Ok(o) => {
                warn!(
                    key,
                    stderr = %String::from_utf8_lossy(&o.stderr),
                    "data_lake upload failed"
                );
            }
            Err(e) => warn!(key, "data_lake upload command error: {e}"),
        }
        return Ok(());
    }

    warn!("data_lake: unknown destination scheme: {destination}");
    Ok(())
}

/// Spawn a retention-enforcement worker that deletes LLM logs older than
/// the per-tenant retention period (or a global default).
pub fn spawn_retention_worker(
    log_repo: Arc<gateway_db::repository::LlmLogRepository>,
    setting_repo: Arc<gateway_db::repository::SettingRepository>,
    tenant_repo: Arc<gateway_db::repository::TenantRepository>,
    default_retention_days: i32,
    interval_hours: u64,
) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_hours * 3600));
        ticker.tick().await; // skip first
        loop {
            ticker.tick().await;
            let tenants = match tenant_repo.list().await {
                Ok(ts) => ts,
                Err(e) => {
                    warn!("retention: could not list tenants: {e}");
                    continue;
                }
            };
            for tenant in tenants {
                let days = setting_repo
                    .get(tenant.id, "llm_log_retention_days")
                    .await
                    .ok()
                    .flatten()
                    .and_then(|s| s.value.parse::<i32>().ok())
                    .unwrap_or(default_retention_days);
                if days <= 0 {
                    continue;
                }
                match log_repo.delete_older_than(tenant.id, days).await {
                    Ok(n) if n > 0 => info!(tenant_id = %tenant.id, days, rows = n, "retention: deleted old logs"),
                    Ok(_) => {}
                    Err(e) => warn!(tenant_id = %tenant.id, "retention delete failed: {e}"),
                }
            }
        }
    });
}
