use std::sync::Arc;
use uuid::Uuid;

use crate::{
    ip_filter::IpFilter,
    budget::BudgetEnforcer,
    rate_limiter::{RateLimiter, RateLimitKey},
    error::PolicyError,
};

/// Ordered policy evaluation engine.
///
/// Evaluation order:
/// 1. IP Filter (global + tenant)
/// 2. Request Body Size
/// 3. GraphQL Depth Check
/// 4. Budget Check (daily per tenant, monthly per tenant)
/// 5. Rate Limiting (per API key > per user > per tenant)
pub struct PolicyEngine {
    pub ip_filter: Arc<IpFilter>,
    pub budget_enforcer: Arc<BudgetEnforcer>,
    pub rate_limiter: Arc<RateLimiter>,
}

impl PolicyEngine {
    pub fn new(
        ip_filter: Arc<IpFilter>,
        budget_enforcer: Arc<BudgetEnforcer>,
        rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        Self {
            ip_filter,
            budget_enforcer,
            rate_limiter,
        }
    }

    /// Evaluate all active policies for a request.
    #[allow(clippy::too_many_arguments)]
    pub async fn evaluate(
        &self,
        ip: &str,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        api_key_id: Option<Uuid>,
        rpm_limit: u32,
        soft_budget: f64,
        hard_budget: f64,
        estimated_cost: f64,
        max_body_size: Option<usize>,
        current_body_size: usize,
        graphql_depth_limit: Option<u32>,
        estimated_depth: Option<u32>,
    ) -> Result<(), PolicyError> {
        // 1. IP Filter (global + per-tenant)
        self.ip_filter.check_for_tenant(ip, tenant_id)?;

        // 2. Body Size Check
        if let Some(limit) = max_body_size {
            if current_body_size > limit {
                return Err(PolicyError::RequestTooLarge {
                    size: current_body_size,
                    max: limit,
                });
            }
        }

        // 3. GraphQL Depth Check
        if let (Some(limit), Some(depth)) = (graphql_depth_limit, estimated_depth) {
            if depth > limit {
                return Err(PolicyError::GraphqlDepthExceeded { depth, limit });
            }
        }

        // 4. Budget Check (daily) — async, Redis-safe
        if hard_budget > 0.0 || soft_budget > 0.0 {
            self.budget_enforcer.check(tenant_id, estimated_cost, soft_budget, hard_budget).await?;
        }

        // 5. Rate Limiting — most specific key wins
        let rl_key = if let Some(id) = api_key_id {
            RateLimitKey::ApiKey(id)
        } else if let Some(id) = user_id {
            RateLimitKey::User(id)
        } else {
            RateLimitKey::Tenant(tenant_id)
        };

        self.rate_limiter.check(&rl_key, rpm_limit).await?;

        Ok(())
    }

    /// Evaluate with model-specific rate limiting (for LLM endpoints).
    #[allow(clippy::too_many_arguments)]
    pub async fn evaluate_with_model(
        &self,
        ip: &str,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        api_key_id: Option<Uuid>,
        rpm_limit: u32,
        model: &str,
        model_rpm_limit: Option<u32>,
        soft_budget: f64,
        hard_budget: f64,
        estimated_cost: f64,
    ) -> Result<(), PolicyError> {
        // IP + budget (no body/graphql for LLM)
        self.ip_filter.check_for_tenant(ip, tenant_id)?;

        if hard_budget > 0.0 || soft_budget > 0.0 {
            self.budget_enforcer.check(tenant_id, estimated_cost, soft_budget, hard_budget).await?;
        }

        // Entity-level rate limit
        let rl_key = if let Some(id) = api_key_id {
            RateLimitKey::ApiKey(id)
        } else if let Some(id) = user_id {
            RateLimitKey::User(id)
        } else {
            RateLimitKey::Tenant(tenant_id)
        };
        self.rate_limiter.check(&rl_key, rpm_limit).await?;

        // Model-level rate limit (if configured)
        if let Some(model_rpm) = model_rpm_limit {
            let model_key = if let Some(uid) = user_id {
                RateLimitKey::user_model(uid, model)
            } else {
                RateLimitKey::tenant_model(tenant_id, model)
            };
            self.rate_limiter.check(&model_key, model_rpm).await?;
        }

        Ok(())
    }

    /// Record usage after a successful request (fire-and-forget; errors are
    /// non-fatal and already logged inside `BudgetEnforcer::record_entity`).
    pub fn record_usage(&self, tenant_id: Uuid, actual_cost: f64) {
        let enforcer = self.budget_enforcer.clone();
        tokio::spawn(async move {
            enforcer.record(tenant_id, actual_cost).await;
        });
    }

    /// Record usage with per-user and per-key attribution.
    pub fn record_usage_detailed(
        &self,
        tenant_id: Uuid,
        user_id: Option<Uuid>,
        api_key_id: Option<Uuid>,
        actual_cost: f64,
    ) {
        let enforcer = self.budget_enforcer.clone();
        tokio::spawn(async move {
            enforcer.record(tenant_id, actual_cost).await;
            if let Some(uid) = user_id {
                enforcer.record_user(uid, actual_cost).await;
            }
            if let Some(kid) = api_key_id {
                enforcer.record_api_key(kid, actual_cost).await;
            }
        });
    }
}
