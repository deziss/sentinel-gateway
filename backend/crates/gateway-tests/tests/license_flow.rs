//! Integration tests for license validation, feature flags, deployment modes.

use gateway_license::{
    features::{FeatureFlags, Plan},
    fingerprint,
};

// ── Feature Flags per Plan ─────────────────────────────────────────────────

#[test]
fn community_plan_has_generous_limits() {
    let f = FeatureFlags::for_plan(Plan::Community);
    // OSS self-hosted: unlimited users, backends, requests.
    assert_eq!(f.max_backends, u32::MAX);
    assert_eq!(f.max_users, u32::MAX);
    assert_eq!(f.max_requests_per_month, u64::MAX);
    // But paid features are gated off.
    assert!(!f.grpc_enabled);
    assert!(!f.sso_enabled);
    assert!(!f.multi_tenant);
    assert!(!f.logs_enabled);
    assert!(!f.prompt_templates_enabled);
}

#[test]
fn professional_plan_enables_observability_and_guardrails() {
    let f = FeatureFlags::for_plan(Plan::Professional);
    // Pro: observability, prompts, guardrails, RBAC, teams.
    assert!(f.logs_enabled);
    assert!(f.feedback_enabled);
    assert!(f.prompt_templates_enabled);
    assert!(f.pii_redaction_enabled);
    assert!(f.rbac_enabled);
    assert!(f.team_management);
    assert!(f.multi_tenant);
    // Pro still lacks Enterprise-only features.
    assert!(!f.sso_enabled);
    assert!(!f.audit_logs_enabled);
    assert!(!f.org_management_enabled);
    // 100K/month request cap.
    assert_eq!(f.max_requests_per_month, 100_000);
}

#[test]
fn enterprise_plan_enables_everything() {
    let f = FeatureFlags::for_plan(Plan::Enterprise);
    assert!(f.sso_enabled);
    assert!(f.grpc_enabled);
    assert!(f.multi_tenant);
    assert!(f.custom_branding_enabled);
    assert!(f.http3_enabled);
    assert!(f.model_federation_enabled);
    assert_eq!(f.max_backends, u32::MAX);
}

#[test]
fn plan_from_str_is_case_insensitive() {
    assert_eq!(Plan::from_str("COMMUNITY"), Plan::Community);
    assert_eq!(Plan::from_str("Professional"), Plan::Professional);
    assert_eq!(Plan::from_str("enterprise"), Plan::Enterprise);
    assert_eq!(Plan::from_str("unknown"), Plan::Community);
}

#[test]
fn retention_scales_with_plan() {
    // Community: no retention (OSS; customer runs their own DB).
    assert_eq!(FeatureFlags::for_plan(Plan::Community).retention_days, 0);
    // Pro: fixed 30 days.
    assert_eq!(FeatureFlags::for_plan(Plan::Professional).retention_days, 30);
    // Enterprise: unlimited / custom.
    assert_eq!(FeatureFlags::for_plan(Plan::Enterprise).retention_days, u32::MAX);
}

// ── Hardware Fingerprint ───────────────────────────────────────────────────

#[test]
fn fingerprint_is_deterministic() {
    let f1 = fingerprint::generate_fingerprint(Some("instance-123"));
    let f2 = fingerprint::generate_fingerprint(Some("instance-123"));
    assert_eq!(f1, f2);
}

#[test]
fn fingerprint_differs_by_instance_id() {
    let f1 = fingerprint::generate_fingerprint(Some("instance-A"));
    let f2 = fingerprint::generate_fingerprint(Some("instance-B"));
    assert_ne!(f1, f2);
}

#[test]
fn fingerprint_is_64_hex_chars() {
    let f = fingerprint::generate_fingerprint(None);
    assert_eq!(f.len(), 64);
    assert!(f.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn fingerprint_verify_matches_generation() {
    let id = Some("test-instance");
    let f = fingerprint::generate_fingerprint(id);
    assert!(fingerprint::verify_fingerprint(&f, id));
    assert!(!fingerprint::verify_fingerprint("wrong-hash", id));
}
