//! CEL-based token cost expressions for weighted rate limiting.
//!
//! Enables per-tenant / per-key cost functions that account for the fact that
//! LLM requests vary 100x in cost per call. Input vs output tokens are priced
//! differently; reasoning and cached tokens have yet another rate.
//!
//! # Example expressions
//!
//! ```text
//! // Uniform (treat all tokens equally):
//! input + output
//!
//! // OpenAI-style (output ~3x input):
//! input * 1.0 + output * 3.0
//!
//! // Discount cached tokens (OpenAI/Anthropic prompt caching):
//! input * 1.0 + output * 3.0 + cached * 0.1 - cache_creation * 0.0
//!
//! // Penalize reasoning tokens (o1, Claude 3.7 thinking):
//! input + output * 3.0 + reasoning * 5.0
//!
//! // Model-aware:
//! model == "gpt-4o" ? input * 2.5 + output * 10.0 : input * 0.5 + output * 1.5
//! ```
//!
//! # Variables available in expressions
//!
//! - `input` / `prompt_tokens` (u64)
//! - `output` / `completion_tokens` (u64)
//! - `cached` / `cached_tokens` (u64)
//! - `cache_creation` / `cache_creation_tokens` (u64)
//! - `reasoning` / `reasoning_tokens` (u64)
//! - `total` / `total_tokens` (u64)
//! - `model` (string)
//! - `tenant` (string)
//!
//! All expressions are parsed once at load time, then evaluated per-request —
//! cheap (~1µs per evaluation).

use cel_interpreter::{Context, Program, Value as CelValue};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

use crate::rate_limiter::{RateLimitKey, RateLimitResult, RateLimiter};
use crate::error::PolicyError;

#[derive(Debug, Error)]
pub enum CelError {
    #[error("Failed to parse expression: {0}")]
    ParseError(String),
    #[error("Evaluation error: {0}")]
    EvalError(String),
    #[error("Expression did not return a number: got {0}")]
    NotNumeric(String),
}

/// A parsed, cacheable CEL cost expression.
/// Parse once at config-load time, clone for evaluation.
#[derive(Clone)]
pub struct CostExpression {
    source: String,
    program: Arc<Program>,
}

impl std::fmt::Debug for CostExpression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CostExpression")
            .field("source", &self.source)
            .finish()
    }
}

impl CostExpression {
    /// Parse a CEL expression. Fails at config time if the syntax is invalid —
    /// this keeps bad configs from crashing request handlers.
    pub fn parse(source: impl Into<String>) -> Result<Self, CelError> {
        let source = source.into();
        let program = Program::compile(&source).map_err(|e| CelError::ParseError(e.to_string()))?;
        Ok(Self { source, program: Arc::new(program) })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// Evaluate the expression with the supplied variable bindings.
    /// Returns the computed cost (rounded to u64).
    pub fn evaluate(&self, vars: &TokenVars) -> Result<u64, CelError> {
        let mut ctx = Context::default();

        // Canonical names
        ctx.add_variable_from_value("input", vars.input);
        ctx.add_variable_from_value("output", vars.output);
        ctx.add_variable_from_value("cached", vars.cached);
        ctx.add_variable_from_value("cache_creation", vars.cache_creation);
        ctx.add_variable_from_value("reasoning", vars.reasoning);
        ctx.add_variable_from_value("total", vars.input + vars.output);

        // OpenAI-style aliases (developers switching from other platforms)
        ctx.add_variable_from_value("prompt_tokens", vars.input);
        ctx.add_variable_from_value("completion_tokens", vars.output);
        ctx.add_variable_from_value("cached_tokens", vars.cached);
        ctx.add_variable_from_value("cache_creation_tokens", vars.cache_creation);
        ctx.add_variable_from_value("reasoning_tokens", vars.reasoning);
        ctx.add_variable_from_value("total_tokens", vars.input + vars.output);

        // Context
        ctx.add_variable_from_value("model", vars.model.clone());
        ctx.add_variable_from_value("tenant", vars.tenant.clone());

        let result = self
            .program
            .execute(&ctx)
            .map_err(|e| CelError::EvalError(e.to_string()))?;

        match result {
            CelValue::Int(n) => Ok(n.max(0) as u64),
            CelValue::UInt(n) => Ok(n),
            CelValue::Float(n) => Ok(n.max(0.0).round() as u64),
            other => Err(CelError::NotNumeric(format!("{other:?}"))),
        }
    }
}

/// Variables passed to a CEL cost expression.
#[derive(Debug, Clone, Default)]
pub struct TokenVars {
    pub input: i64,
    pub output: i64,
    pub cached: i64,
    pub cache_creation: i64,
    pub reasoning: i64,
    pub model: String,
    pub tenant: String,
}

impl TokenVars {
    pub fn new(input: u64, output: u64, model: impl Into<String>) -> Self {
        Self {
            input: input as i64,
            output: output as i64,
            cached: 0,
            cache_creation: 0,
            reasoning: 0,
            model: model.into(),
            tenant: String::new(),
        }
    }

    pub fn with_cached(mut self, cached: u64, cache_creation: u64) -> Self {
        self.cached = cached as i64;
        self.cache_creation = cache_creation as i64;
        self
    }

    pub fn with_reasoning(mut self, reasoning: u64) -> Self {
        self.reasoning = reasoning as i64;
        self
    }

    pub fn with_tenant(mut self, tenant: impl Into<String>) -> Self {
        self.tenant = tenant.into();
        self
    }
}

// ── Extension on RateLimiter ───────────────────────────────────────────────

/// Config for CEL-driven token rate limits. One per (tenant, scope).
#[derive(Debug, Clone)]
pub struct CelRateLimit {
    /// Parsed cost expression.
    pub expression: CostExpression,
    /// Max cost units per minute.
    pub limit_per_minute: u32,
    /// Optional — caption shown in error messages / metrics.
    pub label: String,
}

impl CelRateLimit {
    /// Evaluate the expression, then debit the result from the rate limiter.
    /// Returns an allowed/blocked result — caller typically wants to return
    /// 429 Too Many Requests when `allowed = false`.
    pub async fn consume(
        &self,
        limiter: &RateLimiter,
        key: &RateLimitKey,
        vars: &TokenVars,
    ) -> Result<RateLimitResult, PolicyError> {
        let cost = self
            .expression
            .evaluate(vars)
            .map_err(|e| PolicyError::Internal(format!("CEL eval: {e}")))?;

        limiter.consume(key, self.limit_per_minute, cost).await
    }
}

/// Cache of parsed expressions keyed by (tenant, scope). Avoids re-parsing
/// on every request.
#[derive(Clone, Default)]
pub struct CelRateLimitRegistry {
    by_key: Arc<parking_lot::RwLock<HashMap<String, CelRateLimit>>>,
}

impl CelRateLimitRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register (or replace) a named CEL rate limit.
    /// Returns an error if the expression fails to parse.
    pub fn set(
        &self,
        name: impl Into<String>,
        expression: &str,
        limit_per_minute: u32,
        label: impl Into<String>,
    ) -> Result<(), CelError> {
        let expr = CostExpression::parse(expression)?;
        let name = name.into();
        self.by_key.write().insert(
            name,
            CelRateLimit {
                expression: expr,
                limit_per_minute,
                label: label.into(),
            },
        );
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<CelRateLimit> {
        self.by_key.read().get(name).cloned()
    }

    pub fn remove(&self, name: &str) {
        self.by_key.write().remove(name);
    }

    pub fn len(&self) -> usize {
        self.by_key.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_key.read().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_sum() {
        let expr = CostExpression::parse("input + output").unwrap();
        let vars = TokenVars::new(100, 200, "gpt-4o");
        assert_eq!(expr.evaluate(&vars).unwrap(), 300);
    }

    #[test]
    fn weighted_cost() {
        let expr = CostExpression::parse("input * 1 + output * 3").unwrap();
        let vars = TokenVars::new(100, 200, "gpt-4o");
        // 100 * 1 + 200 * 3 = 700
        assert_eq!(expr.evaluate(&vars).unwrap(), 700);
    }

    #[test]
    fn openai_aliases_work() {
        let expr = CostExpression::parse("prompt_tokens + completion_tokens * 2").unwrap();
        let vars = TokenVars::new(50, 25, "any");
        // 50 + 25*2 = 100
        assert_eq!(expr.evaluate(&vars).unwrap(), 100);
    }

    #[test]
    fn cached_tokens_discounted() {
        // Charge input at full rate, cached at 10% rate, output at 3x
        let expr = CostExpression::parse(
            "(input - cached) * 1 + cached * 0 + output * 3",
        )
        .unwrap();
        let vars = TokenVars::new(1000, 100, "gpt-4o").with_cached(800, 0);
        // (1000-800)*1 + 800*0 + 100*3 = 200 + 0 + 300 = 500
        assert_eq!(expr.evaluate(&vars).unwrap(), 500);
    }

    #[test]
    fn reasoning_penalty() {
        let expr = CostExpression::parse("input + output * 3 + reasoning * 10").unwrap();
        let vars = TokenVars::new(100, 50, "o1").with_reasoning(1000);
        // 100 + 50*3 + 1000*10 = 100 + 150 + 10000 = 10250
        assert_eq!(expr.evaluate(&vars).unwrap(), 10250);
    }

    #[test]
    fn model_aware_conditional() {
        let expr = CostExpression::parse(
            r#"model == "gpt-4o" ? input * 5 + output * 15 : input + output"#,
        )
        .unwrap();

        let premium = TokenVars::new(100, 50, "gpt-4o");
        assert_eq!(expr.evaluate(&premium).unwrap(), 1250); // 500 + 750

        let cheap = TokenVars::new(100, 50, "gpt-3.5-turbo");
        assert_eq!(expr.evaluate(&cheap).unwrap(), 150); // 100 + 50
    }

    #[test]
    fn invalid_expression_fails_at_parse() {
        assert!(CostExpression::parse("this is not cel (((").is_err());
    }

    #[test]
    fn registry_caches_parsed_expressions() {
        let reg = CelRateLimitRegistry::new();
        reg.set("default", "input + output * 2", 10_000, "per-minute")
            .unwrap();

        let cel = reg.get("default").unwrap();
        let vars = TokenVars::new(100, 50, "any");
        assert_eq!(cel.expression.evaluate(&vars).unwrap(), 200);
        assert_eq!(cel.limit_per_minute, 10_000);

        reg.remove("default");
        assert!(reg.get("default").is_none());
    }
}
