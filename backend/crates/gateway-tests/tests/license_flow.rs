//! Integration tests for license validation, feature flags, deployment modes.

use gateway_license::{
    features::{FeatureFlags, Plan},
    fingerprint,
};

// ── Feature Flags per Plan ─────────────────────────────────────────────────

#[test]
fn community_plan_has_generous_limits() {
    let f = FeatureFlags::for_plan(Plan::Community);
    assert_eq!(f.max_backends, u32::MAX);
    assert_eq!(f.max_users, u32::MAX);
    assert!(f.graphql_enabled);
    assert!(!f.grpc_enabled);
    assert!(!f.sso_enabled);
    assert!(!f.multi_tenant);
}

#[test]
fn professional_plan_enables_sso_and_multi_tenant() {
    let f = FeatureFlags::for_plan(Plan::Professional);
    assert!(f.sso_enabled);
    assert!(f.multi_tenant);
    assert!(!f.grpc_enabled);
    assert_eq!(f.max_backends, 20);
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
fn audit_retention_scales_with_plan() {
    assert_eq!(FeatureFlags::for_plan(Plan::Community).audit_log_retention_days, 30);
    assert_eq!(FeatureFlags::for_plan(Plan::Professional).audit_log_retention_days, 90);
    assert_eq!(FeatureFlags::for_plan(Plan::Enterprise).audit_log_retention_days, 365);
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
