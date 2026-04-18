//! Integration tests for authentication flow.
//!
//! Covers: JWT issuance, password hashing, role hierarchy, token blacklist,
//! API key generation/validation, account lockout.
//!
//! These tests are pure-Rust (no DB required) and always run.

use gateway_auth::{
    api_key::ApiKeyService,
    jwt::JwtService,
    password::PasswordService,
    roles::Role,
    token_blacklist::TokenBlacklist,
};
use std::time::Duration;
use uuid::Uuid;

const PRIVATE_KEY_PEM: &[u8] = include_bytes!("../../../keys/private.pem.example");
const PUBLIC_KEY_PEM: &[u8] = include_bytes!("../../../keys/public.pem.example");

fn test_jwt_service() -> JwtService {
    JwtService::new(PRIVATE_KEY_PEM, PUBLIC_KEY_PEM, 15, 7)
        .expect("Failed to init JWT service with example keys")
}

// ── JWT ────────────────────────────────────────────────────────────────────

#[test]
fn jwt_issue_and_validate_roundtrip() {
    let jwt = test_jwt_service();
    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();

    let token = jwt
        .issue_access_token(user_id, tenant_id, Role::User)
        .expect("issue token");

    let claims = jwt.validate(&token).expect("validate token");
    assert_eq!(claims.user_id(), user_id);
    assert_eq!(claims.tenant_id(), tenant_id);
    assert_eq!(claims.role, Role::User);
    assert_eq!(claims.typ, "access");
    assert!(!claims.jti.is_empty());
}

#[test]
fn jwt_refresh_token_has_different_type() {
    let jwt = test_jwt_service();
    let token = jwt
        .issue_refresh_token(Uuid::new_v4(), Uuid::new_v4(), Role::User)
        .unwrap();

    let claims = jwt.validate(&token).unwrap();
    assert_eq!(claims.typ, "refresh");
}

#[test]
fn jwt_rejects_tampered_token() {
    let jwt = test_jwt_service();
    let token = jwt
        .issue_access_token(Uuid::new_v4(), Uuid::new_v4(), Role::User)
        .unwrap();

    // Tamper by flipping a character in the signature portion
    let mut tampered = token.clone();
    let last_char = tampered.pop().unwrap();
    tampered.push(if last_char == 'A' { 'B' } else { 'A' });

    assert!(jwt.validate(&tampered).is_err());
}

// ── Token Blacklist ────────────────────────────────────────────────────────

#[test]
fn token_blacklist_revokes_and_expires() {
    let blacklist = TokenBlacklist::new();
    let jti = "test-jti-123";

    assert!(!blacklist.is_revoked(jti));
    blacklist.revoke(jti, Duration::from_secs(60));
    assert!(blacklist.is_revoked(jti));
}

#[test]
fn token_blacklist_cleanup_removes_expired() {
    let blacklist = TokenBlacklist::new();
    blacklist.revoke("jti-1", Duration::from_millis(1));
    blacklist.revoke("jti-2", Duration::from_secs(60));

    std::thread::sleep(Duration::from_millis(10));
    blacklist.cleanup();

    // jti-1 should be expired (and removed), jti-2 still active
    assert!(!blacklist.is_revoked("jti-1"));
    assert!(blacklist.is_revoked("jti-2"));
}

// ── Password ───────────────────────────────────────────────────────────────

#[test]
fn password_hash_and_verify_roundtrip() {
    let hash = PasswordService::hash("MyS3cur3P@ssw0rd").unwrap();
    assert!(PasswordService::verify("MyS3cur3P@ssw0rd", &hash).unwrap());
    assert!(!PasswordService::verify("wrong", &hash).unwrap());
}

#[test]
fn password_hash_is_salted() {
    let h1 = PasswordService::hash("same_password").unwrap();
    let h2 = PasswordService::hash("same_password").unwrap();
    // Argon2 salted: same input → different hashes
    assert_ne!(h1, h2);
    assert!(PasswordService::verify("same_password", &h1).unwrap());
    assert!(PasswordService::verify("same_password", &h2).unwrap());
}

#[test]
fn password_verify_rejects_malformed_hash() {
    let result = PasswordService::verify("any", "not-a-hash");
    assert!(result.is_err());
}

// ── Roles ──────────────────────────────────────────────────────────────────

#[test]
fn role_hierarchy_is_ordered() {
    assert!(Role::SuperAdmin.can(&Role::TenantAdmin));
    assert!(Role::SuperAdmin.can(&Role::User));
    assert!(Role::SuperAdmin.can(&Role::ReadOnly));
    assert!(Role::TenantAdmin.can(&Role::User));
    assert!(!Role::User.can(&Role::TenantAdmin));
    assert!(!Role::ReadOnly.can(&Role::User));
}

#[test]
fn role_helpers() {
    assert!(Role::SuperAdmin.is_super_admin());
    assert!(!Role::TenantAdmin.is_super_admin());
    assert!(Role::TenantAdmin.is_at_least_tenant_admin());
    assert!(Role::SuperAdmin.is_at_least_tenant_admin());
    assert!(!Role::User.is_at_least_tenant_admin());
}

// ── API Keys ───────────────────────────────────────────────────────────────

#[test]
fn api_key_generate_has_correct_format() {
    let (plaintext, hash) = ApiKeyService::generate();
    assert!(plaintext.starts_with("sg_"), "Plaintext should start with sg_: {plaintext}");
    assert_eq!(hash.len(), 64, "SHA-256 hash should be 64 hex chars");
    assert_eq!(ApiKeyService::hash(&plaintext), hash);
}

#[test]
fn api_key_different_keys_have_different_hashes() {
    let (k1, h1) = ApiKeyService::generate();
    let (k2, h2) = ApiKeyService::generate();
    assert_ne!(k1, k2);
    assert_ne!(h1, h2);
}

#[test]
fn api_key_extract_from_bearer_header() {
    let extracted = ApiKeyService::extract_from_header("Bearer sg_abc123");
    assert_eq!(extracted, Some("sg_abc123"));
}

#[test]
fn api_key_extract_raw_sg_prefix() {
    let extracted = ApiKeyService::extract_from_header("sg_raw_key");
    assert_eq!(extracted, Some("sg_raw_key"));
}

#[test]
fn api_key_rejects_non_sg_prefix() {
    let extracted = ApiKeyService::extract_from_header("some-other-value");
    assert_eq!(extracted, None);
}
