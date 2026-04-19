use dashmap::DashMap;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,   // Normal operation
    Open,     // Failing, reject requests
    HalfOpen, // Testing if backend recovered
}

struct BreakerEntry {
    state: CircuitState,
    failures: u32,
    last_failure: Instant,
}

/// Per-backend circuit breaker.
pub struct CircuitBreaker {
    breakers: DashMap<Uuid, BreakerEntry>,
    failure_threshold: u32,
    recovery_timeout: Duration,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, recovery_timeout_secs: u64) -> Self {
        Self {
            breakers: DashMap::new(),
            failure_threshold,
            recovery_timeout: Duration::from_secs(recovery_timeout_secs),
        }
    }

    pub fn is_open(&self, backend_id: Uuid) -> bool {
        if let Some(mut entry) = self.breakers.get_mut(&backend_id) {
            if entry.state == CircuitState::Open {
                if entry.last_failure.elapsed() > self.recovery_timeout {
                    entry.state = CircuitState::HalfOpen;
                    return false;
                }
                return true;
            }
        }
        false
    }

    pub fn record_success(&self, backend_id: Uuid) {
        if let Some(mut entry) = self.breakers.get_mut(&backend_id) {
            entry.state = CircuitState::Closed;
            entry.failures = 0;
        }
    }

    pub fn record_failure(&self, backend_id: Uuid) {
        let mut entry = self.breakers.entry(backend_id).or_insert(BreakerEntry {
            state: CircuitState::Closed,
            failures: 0,
            last_failure: Instant::now(),
        });
        entry.failures += 1;
        entry.last_failure = Instant::now();
        if entry.failures >= self.failure_threshold {
            entry.state = CircuitState::Open;
        }
    }
}
