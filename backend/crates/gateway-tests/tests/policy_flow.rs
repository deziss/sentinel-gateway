//! Integration tests for policy engine: rate limiting, budgets, IP filter.

use gateway_policy::{
    budget::{BudgetEnforcer, BudgetPeriod, BudgetStatus},
    ip_filter::IpFilter,
    rate_limiter::{RateLimitKey, RateLimiter},
};
use uuid::Uuid;

// ── Rate Limiter (Token Bucket) ────────────────────────────────────────────

#[tokio::test]
async fn rate_limiter_allows_within_limit() {
    let rl = RateLimiter::new_in_memory();
    let key = RateLimitKey::User(Uuid::new_v4());

    // 60 req/min bucket — first 10 should pass instantly
    for _ in 0..10 {
        assert!(rl.check(&key, 60).await.is_ok());
    }
}

#[tokio::test]
async fn rate_limiter_blocks_when_exhausted() {
    let rl = RateLimiter::new_in_memory();
    let key = RateLimitKey::User(Uuid::new_v4());

    // 5 req/min bucket — 6th should fail
    for _ in 0..5 {
        assert!(rl.check(&key, 5).await.is_ok());
    }
    let result = rl.check(&key, 5).await;
    assert!(result.is_err(), "6th request must be rate-limited");
}

#[tokio::test]
async fn rate_limiter_isolates_keys() {
    let rl = RateLimiter::new_in_memory();
    let user1 = RateLimitKey::User(Uuid::new_v4());
    let user2 = RateLimitKey::User(Uuid::new_v4());

    // Exhaust user1
    for _ in 0..5 {
        rl.check(&user1, 5).await.unwrap();
    }
    assert!(rl.check(&user1, 5).await.is_err());
    // user2 is unaffected
    assert!(rl.check(&user2, 5).await.is_ok());
}

#[tokio::test]
async fn rate_limiter_sliding_window() {
    let rl = RateLimiter::new_in_memory_sliding();
    let key = RateLimitKey::Ip("192.0.2.1".to_string());

    for _ in 0..3 {
        assert!(rl.check(&key, 3).await.is_ok());
    }
    assert!(rl.check(&key, 3).await.is_err());
}

#[tokio::test]
async fn rate_limiter_returns_remaining_count() {
    let rl = RateLimiter::new_in_memory();
    let key = RateLimitKey::User(Uuid::new_v4());

    let result = rl.check_detailed(&key, 10).await.unwrap();
    assert!(result.allowed);
    assert_eq!(result.limit, 10);
}

// ── Budget Enforcer ───────────────────────────────────────────────────────

#[tokio::test]
async fn budget_allows_within_limit() {
    let enf = BudgetEnforcer::new();
    let tenant = Uuid::new_v4();

    let status = enf.check(tenant, 0.50, 5.0, 10.0).await.unwrap();
    assert!(matches!(status, BudgetStatus::WithinLimits { .. }));
}

#[tokio::test]
async fn budget_warns_at_soft_limit() {
    let enf = BudgetEnforcer::new();
    let tenant = Uuid::new_v4();
    enf.record(tenant, 5.0).await;

    let status = enf.check(tenant, 0.50, 5.0, 10.0).await.unwrap();
    assert!(matches!(status, BudgetStatus::SoftLimitExceeded { .. }));
}

#[tokio::test]
async fn budget_blocks_at_hard_limit() {
    let enf = BudgetEnforcer::new();
    let tenant = Uuid::new_v4();
    enf.record(tenant, 10.0).await;

    let result = enf.check(tenant, 0.50, 5.0, 10.0).await;
    assert!(result.is_err(), "should exceed hard limit");
}

#[tokio::test]
async fn budget_isolates_tenants() {
    let enf = BudgetEnforcer::new();
    let t1 = Uuid::new_v4();
    let t2 = Uuid::new_v4();

    enf.record(t1, 100.0).await;
    // t2 is unaffected
    let status = enf.check(t2, 1.0, 10.0, 50.0).await.unwrap();
    assert!(matches!(status, BudgetStatus::WithinLimits { .. }));
}

#[tokio::test]
async fn budget_tracks_multiple_periods() {
    let enf = BudgetEnforcer::new();
    let tenant = Uuid::new_v4();
    enf.record(tenant, 5.0).await;

    let daily = enf.get_spend(&format!("ten:{tenant}"), BudgetPeriod::Daily).await;
    let monthly = enf.get_spend(&format!("ten:{tenant}"), BudgetPeriod::Monthly).await;

    assert_eq!(daily, 5.0);
    assert_eq!(monthly, 5.0);
}

// ── IP Filter ─────────────────────────────────────────────────────────────

#[test]
fn ip_filter_global_denylist() {
    let filter = IpFilter::new();
    filter.global_deny("192.0.2.1");

    assert!(filter.check("192.0.2.1").is_err());
    assert!(filter.check("192.0.2.2").is_ok());
}

#[test]
fn ip_filter_per_tenant_denylist() {
    let filter = IpFilter::new();
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();

    filter.tenant_deny(tenant_a, "10.0.0.1");

    // Blocked for tenant A
    assert!(filter.check_for_tenant("10.0.0.1", tenant_a).is_err());
    // Allowed for tenant B
    assert!(filter.check_for_tenant("10.0.0.1", tenant_b).is_ok());
}

#[test]
fn ip_filter_per_tenant_allowlist() {
    let filter = IpFilter::new();
    let tenant = Uuid::new_v4();

    filter.tenant_allow(tenant, "10.0.0.5");

    // Only the allowlisted IP passes
    assert!(filter.check_for_tenant("10.0.0.5", tenant).is_ok());
    assert!(filter.check_for_tenant("10.0.0.6", tenant).is_err());
}

#[test]
fn ip_filter_global_trumps_tenant_allow() {
    let filter = IpFilter::new();
    let tenant = Uuid::new_v4();

    filter.global_deny("10.0.0.1");
    filter.tenant_allow(tenant, "10.0.0.1");

    // Global denylist wins
    assert!(filter.check_for_tenant("10.0.0.1", tenant).is_err());
}
