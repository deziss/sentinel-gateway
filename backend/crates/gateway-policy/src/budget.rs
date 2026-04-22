//! Budget enforcer: soft/hard limits per tenant, user, or API key.
//!
//! Supports two backends:
//! - **InMemory** — per-replica DashMap; suitable for single-replica deployments.
//! - **Redis** — INCRBYFLOAT-based atomic counters; required when `GATEWAY__REPLICAS > 1`
//!   to ensure cross-replica budget consistency.
//!
//! Mirrors the dual-backend design of [`crate::rate_limiter::RateLimiter`].

use chrono::{Datelike, Utc};
use dashmap::DashMap;
use fred::prelude::*;
use uuid::Uuid;

use crate::error::PolicyError;

// ── Budget Period ──────────────────────────────────────────────────────────

/// Budget period for tracking spend.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BudgetPeriod {
    Daily,
    Weekly,
    Monthly,
}

impl BudgetPeriod {
    /// Return a deterministic string key for the current period window.
    /// Used both as a DashMap key suffix and as a Redis key component.
    pub fn current_key(&self) -> String {
        let now = Utc::now();
        match self {
            BudgetPeriod::Daily => now.format("%Y-%m-%d").to_string(),
            BudgetPeriod::Weekly => {
                let iso_week = now.iso_week();
                format!("{}-W{:02}", iso_week.year(), iso_week.week())
            }
            BudgetPeriod::Monthly => now.format("%Y-%m").to_string(),
        }
    }

    /// TTL (in seconds) until the end of the current period window.
    /// Used for Redis key expiry so stale counters self-clean.
    pub fn ttl_secs(&self) -> u64 {
        let now = Utc::now();
        match self {
            BudgetPeriod::Daily => {
                // Seconds until midnight UTC
                let next_day = now.date_naive().succ_opt().unwrap_or(now.date_naive());
                let midnight = chrono::NaiveDateTime::new(
                    next_day,
                    chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                );
                let diff = midnight.and_utc() - now;
                diff.num_seconds().max(1) as u64
            }
            BudgetPeriod::Weekly => {
                // Seconds until next Monday 00:00 UTC
                let days_until_monday = (7 - now.weekday().num_days_from_monday()) % 7;
                let next_monday = now.date_naive() + chrono::Duration::days(days_until_monday as i64 + 1);
                let midnight = chrono::NaiveDateTime::new(
                    next_monday,
                    chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                );
                let diff = midnight.and_utc() - now;
                diff.num_seconds().max(1) as u64
            }
            BudgetPeriod::Monthly => {
                // Seconds until start of next month
                let (year, month) = if now.month() == 12 {
                    (now.year() + 1, 1u32)
                } else {
                    (now.year(), now.month() + 1)
                };
                let first = chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap();
                let midnight = chrono::NaiveDateTime::new(
                    first,
                    chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                );
                let diff = midnight.and_utc() - now;
                diff.num_seconds().max(1) as u64
            }
        }
    }
}

// ── Budget Status ──────────────────────────────────────────────────────────

/// Budget status response.
#[derive(Debug, Clone)]
pub enum BudgetStatus {
    WithinLimits { current: f64, limit: f64 },
    SoftLimitExceeded { current: f64, limit: f64 },
    HardLimitExceeded { current: f64, limit: f64 },
}

// ── In-memory composite key ────────────────────────────────────────────────

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct BudgetKey {
    entity: String,
    period_key: String,
}

// ── BudgetEnforcer ────────────────────────────────────────────────────────

/// Budget enforcer — in-memory or Redis backend.
pub enum BudgetEnforcer {
    /// Per-replica DashMap. **Not safe for multi-replica deployments.**
    InMemory {
        spend: DashMap<BudgetKey, f64>,
    },
    /// Redis INCRBYFLOAT-based atomic counters. Safe for multi-replica deployments.
    Redis(RedisClient),
}

impl BudgetEnforcer {
    /// Create an in-memory budget enforcer (single-replica only).
    pub fn new() -> Self {
        Self::InMemory {
            spend: DashMap::new(),
        }
    }

    /// Create a Redis-backed budget enforcer (multi-replica safe).
    pub fn new_redis(client: RedisClient) -> Self {
        Self::Redis(client)
    }

    // ── Internal helpers ───────────────────────────────────────────────────

    fn redis_key(entity: &str, period: &BudgetPeriod) -> String {
        format!("bgt:{}:{}", entity, period.current_key())
    }

    async fn get_spend_redis(client: &RedisClient, key: &str) -> f64 {
        let val: Option<String> = client.get(key).await.unwrap_or(None);
        val.and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0)
    }

    async fn incr_spend_redis(client: &RedisClient, key: &str, cost: f64, period: &BudgetPeriod) {
        // INCRBYFLOAT is atomic. Set EXPIRE only when the key is first created
        // (i.e., current value ≈ cost after the increment).
        let _: Result<f64, _> = client.incr_by_float(key, cost).await;
        // Always refresh expiry in case a previous EXPIRE didn't fire.
        let ttl = period.ttl_secs();
        let _: Result<bool, _> = client.expire(key, ttl as i64).await;
    }

    // ── Public API ─────────────────────────────────────────────────────────

    /// Check tenant daily budget.
    pub async fn check(
        &self,
        tenant_id: Uuid,
        cost: f64,
        soft_limit: f64,
        hard_limit: f64,
    ) -> Result<BudgetStatus, PolicyError> {
        self.check_entity(&format!("ten:{tenant_id}"), BudgetPeriod::Daily, cost, soft_limit, hard_limit).await
    }

    /// Check budget for any entity + period combination.
    pub async fn check_entity(
        &self,
        entity: &str,
        period: BudgetPeriod,
        cost: f64,
        soft_limit: f64,
        hard_limit: f64,
    ) -> Result<BudgetStatus, PolicyError> {
        let current = self.get_spend(entity, period.clone()).await;
        let projected = current + cost;

        if hard_limit > 0.0 && projected > hard_limit {
            return Err(PolicyError::BudgetExceeded(format!(
                "Hard limit ${hard_limit:.2} exceeded (current: ${current:.4}, projected: ${projected:.4})"
            )));
        }

        if soft_limit > 0.0 && projected > soft_limit {
            return Ok(BudgetStatus::SoftLimitExceeded { current, limit: soft_limit });
        }

        Ok(BudgetStatus::WithinLimits { current, limit: hard_limit })
    }

    /// Check daily budget for a user.
    pub async fn check_user_daily(
        &self,
        user_id: Uuid,
        cost: f64,
        soft_limit: f64,
        hard_limit: f64,
    ) -> Result<BudgetStatus, PolicyError> {
        self.check_entity(&format!("usr:{user_id}"), BudgetPeriod::Daily, cost, soft_limit, hard_limit).await
    }

    /// Check monthly budget for a tenant.
    pub async fn check_tenant_monthly(
        &self,
        tenant_id: Uuid,
        cost: f64,
        soft_limit: f64,
        hard_limit: f64,
    ) -> Result<BudgetStatus, PolicyError> {
        self.check_entity(&format!("ten:{tenant_id}"), BudgetPeriod::Monthly, cost, soft_limit, hard_limit).await
    }

    /// Check daily budget for an API key.
    pub async fn check_api_key_daily(
        &self,
        api_key_id: Uuid,
        cost: f64,
        soft_limit: f64,
        hard_limit: f64,
    ) -> Result<BudgetStatus, PolicyError> {
        self.check_entity(&format!("key:{api_key_id}"), BudgetPeriod::Daily, cost, soft_limit, hard_limit).await
    }

    /// Record actual spend after a request completes (all three periods).
    pub async fn record(&self, tenant_id: Uuid, cost: f64) {
        self.record_entity(&format!("ten:{tenant_id}"), cost).await;
    }

    /// Record spend for any entity across all active periods.
    pub async fn record_entity(&self, entity: &str, cost: f64) {
        for period in [BudgetPeriod::Daily, BudgetPeriod::Weekly, BudgetPeriod::Monthly] {
            match self {
                Self::InMemory { spend } => {
                    let key = BudgetKey {
                        entity: entity.to_string(),
                        period_key: period.current_key(),
                    };
                    *spend.entry(key).or_insert(0.0) += cost;
                }
                Self::Redis(client) => {
                    let rkey = Self::redis_key(entity, &period);
                    Self::incr_spend_redis(client, &rkey, cost, &period).await;
                }
            }
        }
    }

    /// Record spend for a user.
    pub async fn record_user(&self, user_id: Uuid, cost: f64) {
        self.record_entity(&format!("usr:{user_id}"), cost).await;
    }

    /// Record spend for an API key.
    pub async fn record_api_key(&self, api_key_id: Uuid, cost: f64) {
        self.record_entity(&format!("key:{api_key_id}"), cost).await;
    }

    /// Get current spend for an entity in a given period.
    pub async fn get_spend(&self, entity: &str, period: BudgetPeriod) -> f64 {
        match self {
            Self::InMemory { spend } => {
                let key = BudgetKey {
                    entity: entity.to_string(),
                    period_key: period.current_key(),
                };
                spend.get(&key).map(|v| *v).unwrap_or(0.0)
            }
            Self::Redis(client) => {
                let rkey = Self::redis_key(entity, &period);
                Self::get_spend_redis(client, &rkey).await
            }
        }
    }

    /// Cleanup old period entries that are no longer current (InMemory only;
    /// Redis keys self-expire via TTL).
    pub fn cleanup_stale(&self) {
        if let Self::InMemory { spend } = self {
            let daily = BudgetPeriod::Daily.current_key();
            let weekly = BudgetPeriod::Weekly.current_key();
            let monthly = BudgetPeriod::Monthly.current_key();

            spend.retain(|key, _| {
                key.period_key == daily || key.period_key == weekly || key.period_key == monthly
            });
        }
        // Redis: no-op — keys self-expire via TTL set in incr_spend_redis
    }
}

impl Default for BudgetEnforcer {
    fn default() -> Self {
        Self::new()
    }
}
