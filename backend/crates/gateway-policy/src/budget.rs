use chrono::{Datelike, Utc};
use dashmap::DashMap;
use uuid::Uuid;

use crate::error::PolicyError;

/// Budget period for tracking spend.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BudgetPeriod {
    Daily,
    Weekly,
    Monthly,
}

impl BudgetPeriod {
    /// Return a string key for the current period (e.g., "2026-04-17" for daily).
    fn current_key(&self) -> String {
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
}

/// Budget status response.
#[derive(Debug, Clone)]
pub enum BudgetStatus {
    WithinLimits { current: f64, limit: f64 },
    SoftLimitExceeded { current: f64, limit: f64 },
    HardLimitExceeded { current: f64, limit: f64 },
}

/// Composite budget key: entity + period.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct BudgetKey {
    entity: String,
    period_key: String,
}

/// Budget enforcer: soft/hard limits per tenant, user, or API key,
/// with daily, weekly, and monthly periods.
pub struct BudgetEnforcer {
    spend: DashMap<BudgetKey, f64>,
}

impl BudgetEnforcer {
    pub fn new() -> Self {
        Self {
            spend: DashMap::new(),
        }
    }

    /// Check if adding `cost` would exceed thresholds for a tenant's daily budget.
    pub fn check(
        &self,
        tenant_id: Uuid,
        cost: f64,
        soft_limit: f64,
        hard_limit: f64,
    ) -> Result<BudgetStatus, PolicyError> {
        self.check_entity(&format!("ten:{tenant_id}"), BudgetPeriod::Daily, cost, soft_limit, hard_limit)
    }

    /// Check budget for any entity + period combination.
    pub fn check_entity(
        &self,
        entity: &str,
        period: BudgetPeriod,
        cost: f64,
        soft_limit: f64,
        hard_limit: f64,
    ) -> Result<BudgetStatus, PolicyError> {
        let key = BudgetKey {
            entity: entity.to_string(),
            period_key: period.current_key(),
        };

        let current = self.spend.get(&key).map(|v| *v).unwrap_or(0.0);
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
    pub fn check_user_daily(
        &self,
        user_id: Uuid,
        cost: f64,
        soft_limit: f64,
        hard_limit: f64,
    ) -> Result<BudgetStatus, PolicyError> {
        self.check_entity(&format!("usr:{user_id}"), BudgetPeriod::Daily, cost, soft_limit, hard_limit)
    }

    /// Check monthly budget for a tenant.
    pub fn check_tenant_monthly(
        &self,
        tenant_id: Uuid,
        cost: f64,
        soft_limit: f64,
        hard_limit: f64,
    ) -> Result<BudgetStatus, PolicyError> {
        self.check_entity(&format!("ten:{tenant_id}"), BudgetPeriod::Monthly, cost, soft_limit, hard_limit)
    }

    /// Check daily budget for an API key.
    pub fn check_api_key_daily(
        &self,
        api_key_id: Uuid,
        cost: f64,
        soft_limit: f64,
        hard_limit: f64,
    ) -> Result<BudgetStatus, PolicyError> {
        self.check_entity(&format!("key:{api_key_id}"), BudgetPeriod::Daily, cost, soft_limit, hard_limit)
    }

    /// Record actual spend after a request completes.
    pub fn record(&self, tenant_id: Uuid, cost: f64) {
        self.record_entity(&format!("ten:{tenant_id}"), cost);
    }

    /// Record spend for any entity across all active periods.
    pub fn record_entity(&self, entity: &str, cost: f64) {
        for period in [BudgetPeriod::Daily, BudgetPeriod::Weekly, BudgetPeriod::Monthly] {
            let key = BudgetKey {
                entity: entity.to_string(),
                period_key: period.current_key(),
            };
            *self.spend.entry(key).or_insert(0.0) += cost;
        }
    }

    /// Record spend for a user.
    pub fn record_user(&self, user_id: Uuid, cost: f64) {
        self.record_entity(&format!("usr:{user_id}"), cost);
    }

    /// Record spend for an API key.
    pub fn record_api_key(&self, api_key_id: Uuid, cost: f64) {
        self.record_entity(&format!("key:{api_key_id}"), cost);
    }

    /// Get current spend for an entity in a period.
    pub fn get_spend(&self, entity: &str, period: BudgetPeriod) -> f64 {
        let key = BudgetKey {
            entity: entity.to_string(),
            period_key: period.current_key(),
        };
        self.spend.get(&key).map(|v| *v).unwrap_or(0.0)
    }

    /// Cleanup old period entries that are no longer current.
    pub fn cleanup_stale(&self) {
        let daily = BudgetPeriod::Daily.current_key();
        let weekly = BudgetPeriod::Weekly.current_key();
        let monthly = BudgetPeriod::Monthly.current_key();

        self.spend.retain(|key, _| {
            key.period_key == daily || key.period_key == weekly || key.period_key == monthly
        });
    }
}

impl Default for BudgetEnforcer {
    fn default() -> Self {
        Self::new()
    }
}
