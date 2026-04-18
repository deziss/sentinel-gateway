use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{ConnectOptions, PgPool};
use std::str::FromStr;
use std::time::Duration;

pub type DbPool = PgPool;

/// Production-ready connection pool with sensible defaults.
///
/// - `max_connections`: hard cap (each connection ~10MB RAM on Postgres side)
/// - `min_connections`: keep-warm pool to avoid cold start penalty
/// - `acquire_timeout`: fail fast if pool exhausted (skill rule: log >1s)
/// - `idle_timeout`: reclaim idle connections after 10 minutes
/// - `max_lifetime`: recycle connections every 30 minutes (prevents stale state)
pub async fn create_pool(database_url: &str, max_connections: u32) -> Result<DbPool, sqlx::Error> {
    create_pool_with_options(database_url, max_connections, false).await
}

/// Create a pool with optional query logging.
/// When `log_queries=true`, SQL statements are emitted at DEBUG level via `tracing`.
/// Slow queries (>200ms) are emitted at WARN level regardless.
pub async fn create_pool_with_options(
    database_url: &str,
    max_connections: u32,
    log_queries: bool,
) -> Result<DbPool, sqlx::Error> {
    let mut opts = PgConnectOptions::from_str(database_url)?;

    // Route SQLx logs through `tracing`
    let stmt_level = if log_queries {
        tracing::log::LevelFilter::Debug
    } else {
        tracing::log::LevelFilter::Off
    };
    opts = opts
        .log_statements(stmt_level)
        .log_slow_statements(tracing::log::LevelFilter::Warn, Duration::from_millis(200));

    PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections((max_connections / 4).max(1))
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Some(Duration::from_secs(600)))
        .max_lifetime(Some(Duration::from_secs(1800)))
        .test_before_acquire(true)
        .connect_with(opts)
        .await
}
