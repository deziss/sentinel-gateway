//! Shared test utilities for gateway integration tests.
//!
//! This crate contains both reusable helpers (exposed via `lib.rs`) and
//! integration tests (in `tests/`). Tests require a PostgreSQL test database
//! — set `TEST_DATABASE_URL` env var or skip via `GATEWAY_SKIP_DB_TESTS=1`.

pub mod fixtures;

/// Check whether DB tests should run. Returns `false` if skipped.
pub fn db_tests_enabled() -> bool {
    std::env::var("GATEWAY_SKIP_DB_TESTS").is_err()
        && std::env::var("TEST_DATABASE_URL").is_ok()
}

/// Get the test database URL, or panic with a helpful message.
pub fn test_db_url() -> String {
    std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://sentinel:sentinel@localhost:5438/sentinel_gateway_test".to_string()
    })
}
