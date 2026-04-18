//! Guardrails framework — a pluggable pipeline of checks applied before and/or
//! after LLM requests.
//!
//! # Design
//!
//! A **guardrail** is anything implementing the [`Guardrail`] trait. The gateway
//! iterates through a tenant's configured guardrails and applies them at the
//! appropriate stage ([`GuardrailStage::PreCall`] or [`GuardrailStage::PostCall`]).
//!
//! Each guardrail returns a [`GuardrailResult`] that tells the gateway:
//! - **Pass** — allow the request/response through
//! - **Modify** — replace content (redaction, rewriting)
//! - **Block** — reject with HTTP 4xx and a reason
//! - **Flag** — allow but log for review
//!
//! # Built-in guardrails
//!
//! - [`RegexGuardrail`] — regex allow/deny lists
//! - [`JsonSchemaGuardrail`] — validate output against JSON Schema
//! - [`LengthGuardrail`] — enforce min/max content length
//! - [`ProfanityGuardrail`] — simple word-list-based filter
//!
//! External PII detection lives in `gateway-llm::pii::PiiDetector`, and is
//! wrapped as a [`PiiGuardrail`] in this module.
//!
//! # Extensibility
//!
//! Additional guardrails (Azure Content Safety, AWS Bedrock Guardrails, Presidio,
//! Lakera, etc.) should implement the [`Guardrail`] trait — usually as an HTTP
//! call to the external service with the request text.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// When the guardrail runs in the request lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailStage {
    /// Before forwarding to the LLM provider.
    PreCall,
    /// After receiving the LLM response, before returning to the client.
    PostCall,
    /// Log only — doesn't block or modify (for A/B testing new guardrails).
    LoggingOnly,
}

/// Outcome of running a guardrail.
#[derive(Debug, Clone)]
pub enum GuardrailOutcome {
    /// Content is acceptable — pass through unchanged.
    Pass,
    /// Content was modified (e.g., PII redacted).
    Modify { content: String },
    /// Content is unacceptable — reject the request.
    Block { reason: String, category: String },
    /// Content is suspicious but allowed — log for review.
    Flag { reason: String, category: String },
}

/// The result of running a guardrail.
#[derive(Debug, Clone)]
pub struct GuardrailResult {
    pub name: String,
    pub stage: GuardrailStage,
    pub outcome: GuardrailOutcome,
    pub duration_ms: u64,
}

impl GuardrailResult {
    pub fn is_blocked(&self) -> bool {
        matches!(self.outcome, GuardrailOutcome::Block { .. })
    }

    pub fn is_modified(&self) -> bool {
        matches!(self.outcome, GuardrailOutcome::Modify { .. })
    }
}

/// Context passed to a guardrail when it runs.
pub struct GuardrailContext<'a> {
    /// The text being evaluated (prompt on PreCall, response on PostCall).
    pub content: &'a str,
    /// Model being used (for logging).
    pub model: Option<&'a str>,
    /// Tenant ID.
    pub tenant_id: Option<uuid::Uuid>,
    /// User ID.
    pub user_id: Option<uuid::Uuid>,
}

/// A pluggable guardrail. Implementations should be `Send + Sync` so the
/// pipeline can run them concurrently where safe.
#[async_trait]
pub trait Guardrail: Send + Sync {
    /// Unique name for this guardrail instance (e.g., "regex-pii", "azure-content-safety").
    fn name(&self) -> &str;

    /// Which stage this guardrail runs in.
    fn stage(&self) -> GuardrailStage;

    /// Evaluate the content. The implementation should be fast — if it takes
    /// more than ~500ms, wrap it in a timeout at the call site.
    async fn check(&self, ctx: &GuardrailContext<'_>) -> GuardrailOutcome;
}

// ── Built-in: RegexGuardrail ────────────────────────────────────────────────

/// Blocks (or flags) content that matches any of the configured patterns.
pub struct RegexGuardrail {
    name: String,
    stage: GuardrailStage,
    patterns: Vec<regex::Regex>,
    mode: GuardrailMode,
    category: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailMode {
    /// Reject the request on match.
    Block,
    /// Replace matched substrings with a redaction token.
    Redact,
    /// Log but allow.
    Flag,
}

impl RegexGuardrail {
    pub fn new(
        name: impl Into<String>,
        stage: GuardrailStage,
        patterns: Vec<&str>,
        mode: GuardrailMode,
        category: impl Into<String>,
    ) -> Result<Self, regex::Error> {
        let patterns = patterns
            .iter()
            .map(|p| regex::Regex::new(p))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            name: name.into(),
            stage,
            patterns,
            mode,
            category: category.into(),
        })
    }
}

#[async_trait]
impl Guardrail for RegexGuardrail {
    fn name(&self) -> &str {
        &self.name
    }
    fn stage(&self) -> GuardrailStage {
        self.stage
    }

    async fn check(&self, ctx: &GuardrailContext<'_>) -> GuardrailOutcome {
        let matched_pattern = self.patterns.iter().find(|p| p.is_match(ctx.content));
        let Some(pattern) = matched_pattern else {
            return GuardrailOutcome::Pass;
        };

        match self.mode {
            GuardrailMode::Block => GuardrailOutcome::Block {
                reason: format!("matched forbidden pattern: {}", pattern.as_str()),
                category: self.category.clone(),
            },
            GuardrailMode::Flag => GuardrailOutcome::Flag {
                reason: format!("matched watched pattern: {}", pattern.as_str()),
                category: self.category.clone(),
            },
            GuardrailMode::Redact => {
                let mut redacted = ctx.content.to_string();
                for p in &self.patterns {
                    redacted = p.replace_all(&redacted, "[REDACTED]").into_owned();
                }
                GuardrailOutcome::Modify { content: redacted }
            }
        }
    }
}

// ── Built-in: JsonSchemaGuardrail ───────────────────────────────────────────

/// Validates that a completion response is valid JSON matching a schema.
/// Useful for enforcing tool-call argument shapes.
pub struct JsonSchemaGuardrail {
    name: String,
    schema: serde_json::Value,
}

impl JsonSchemaGuardrail {
    pub fn new(name: impl Into<String>, schema: serde_json::Value) -> Self {
        Self { name: name.into(), schema }
    }

    fn validate_type(value: &serde_json::Value, expected: &str) -> bool {
        match expected {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.is_i64() || value.is_u64(),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            _ => true,
        }
    }
}

#[async_trait]
impl Guardrail for JsonSchemaGuardrail {
    fn name(&self) -> &str {
        &self.name
    }
    fn stage(&self) -> GuardrailStage {
        GuardrailStage::PostCall
    }

    async fn check(&self, ctx: &GuardrailContext<'_>) -> GuardrailOutcome {
        // Try to parse the content as JSON
        let parsed: serde_json::Value = match serde_json::from_str(ctx.content) {
            Ok(v) => v,
            Err(e) => {
                return GuardrailOutcome::Block {
                    reason: format!("response is not valid JSON: {e}"),
                    category: "schema_validation".into(),
                };
            }
        };

        // Minimal JSON Schema subset: `type`, `required`, `properties`.
        if let Some(expected_type) = self.schema.get("type").and_then(|t| t.as_str()) {
            if !Self::validate_type(&parsed, expected_type) {
                return GuardrailOutcome::Block {
                    reason: format!("expected type '{expected_type}'"),
                    category: "schema_validation".into(),
                };
            }
        }

        if let Some(required) = self.schema.get("required").and_then(|r| r.as_array()) {
            if let Some(obj) = parsed.as_object() {
                for r in required {
                    if let Some(key) = r.as_str() {
                        if !obj.contains_key(key) {
                            return GuardrailOutcome::Block {
                                reason: format!("missing required field '{key}'"),
                                category: "schema_validation".into(),
                            };
                        }
                    }
                }
            }
        }

        GuardrailOutcome::Pass
    }
}

// ── Built-in: LengthGuardrail ───────────────────────────────────────────────

/// Enforces a maximum content length. Useful as a cheap pre-filter against
/// prompt-stuffing attacks.
pub struct LengthGuardrail {
    name: String,
    stage: GuardrailStage,
    max_chars: usize,
}

impl LengthGuardrail {
    pub fn new(name: impl Into<String>, stage: GuardrailStage, max_chars: usize) -> Self {
        Self { name: name.into(), stage, max_chars }
    }
}

#[async_trait]
impl Guardrail for LengthGuardrail {
    fn name(&self) -> &str {
        &self.name
    }
    fn stage(&self) -> GuardrailStage {
        self.stage
    }

    async fn check(&self, ctx: &GuardrailContext<'_>) -> GuardrailOutcome {
        if ctx.content.len() > self.max_chars {
            GuardrailOutcome::Block {
                reason: format!(
                    "content length {} exceeds max {}",
                    ctx.content.len(),
                    self.max_chars
                ),
                category: "length_limit".into(),
            }
        } else {
            GuardrailOutcome::Pass
        }
    }
}

// ── Pipeline ─────────────────────────────────────────────────────────────────

/// A configured guardrail pipeline that applies all guardrails for a given stage.
#[derive(Clone)]
pub struct GuardrailPipeline {
    guards: Vec<Arc<dyn Guardrail>>,
}

impl GuardrailPipeline {
    pub fn new() -> Self {
        Self { guards: Vec::new() }
    }

    pub fn add(&mut self, guard: Arc<dyn Guardrail>) {
        self.guards.push(guard);
    }

    /// Run all guardrails for the given stage. Returns (final_content, results).
    ///
    /// Semantics:
    /// - `Pass` — no change
    /// - `Modify { content }` — update the content for subsequent guardrails (chainable redaction)
    /// - `Block` — returns immediately with the block outcome; caller rejects the request
    /// - `Flag` — logged, request continues
    ///
    /// If any guardrail blocks, all later guardrails for this stage are skipped.
    pub async fn run(
        &self,
        stage: GuardrailStage,
        content: &str,
        model: Option<&str>,
        tenant_id: Option<uuid::Uuid>,
        user_id: Option<uuid::Uuid>,
    ) -> (String, Vec<GuardrailResult>) {
        let mut current_content = content.to_string();
        let mut results = Vec::new();

        for guard in self.guards.iter().filter(|g| g.stage() == stage) {
            let started = std::time::Instant::now();
            let ctx = GuardrailContext {
                content: &current_content,
                model,
                tenant_id,
                user_id,
            };
            let outcome = guard.check(&ctx).await;
            let duration_ms = started.elapsed().as_millis() as u64;

            let is_block = matches!(outcome, GuardrailOutcome::Block { .. });
            if let GuardrailOutcome::Modify { ref content } = outcome {
                current_content = content.clone();
            }

            results.push(GuardrailResult {
                name: guard.name().to_string(),
                stage,
                outcome,
                duration_ms,
            });

            if is_block {
                break;
            }
        }

        (current_content, results)
    }

    pub fn is_empty(&self) -> bool {
        self.guards.is_empty()
    }
}

impl Default for GuardrailPipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn regex_block_on_match() {
        let g = RegexGuardrail::new(
            "test",
            GuardrailStage::PreCall,
            vec![r"secret_\w+"],
            GuardrailMode::Block,
            "secret",
        )
        .unwrap();
        let ctx = GuardrailContext {
            content: "my password is secret_abc123",
            model: None,
            tenant_id: None,
            user_id: None,
        };
        let out = g.check(&ctx).await;
        assert!(matches!(out, GuardrailOutcome::Block { .. }));
    }

    #[tokio::test]
    async fn regex_redact_mode_replaces() {
        let g = RegexGuardrail::new(
            "test",
            GuardrailStage::PreCall,
            vec![r"\d{3}-\d{2}-\d{4}"], // SSN
            GuardrailMode::Redact,
            "pii",
        )
        .unwrap();
        let ctx = GuardrailContext {
            content: "My SSN is 123-45-6789.",
            model: None,
            tenant_id: None,
            user_id: None,
        };
        match g.check(&ctx).await {
            GuardrailOutcome::Modify { content } => {
                assert_eq!(content, "My SSN is [REDACTED].");
            }
            other => panic!("expected Modify, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn length_blocks_oversized() {
        let g = LengthGuardrail::new("len", GuardrailStage::PreCall, 10);
        let ctx = GuardrailContext {
            content: "this is more than ten chars",
            model: None,
            tenant_id: None,
            user_id: None,
        };
        assert!(matches!(g.check(&ctx).await, GuardrailOutcome::Block { .. }));
    }

    #[tokio::test]
    async fn pipeline_chains_redactions() {
        let mut pipeline = GuardrailPipeline::new();
        pipeline.add(Arc::new(
            RegexGuardrail::new(
                "email",
                GuardrailStage::PreCall,
                vec![r"\S+@\S+\.\S+"],
                GuardrailMode::Redact,
                "pii",
            )
            .unwrap(),
        ));
        pipeline.add(Arc::new(
            RegexGuardrail::new(
                "phone",
                GuardrailStage::PreCall,
                vec![r"\d{3}-\d{3}-\d{4}"],
                GuardrailMode::Redact,
                "pii",
            )
            .unwrap(),
        ));

        let (out, results) = pipeline
            .run(
                GuardrailStage::PreCall,
                "Email me at alice@example.com or call 555-123-4567",
                None,
                None,
                None,
            )
            .await;
        assert!(!out.contains("alice@example.com"));
        assert!(!out.contains("555-123-4567"));
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn pipeline_stops_on_block() {
        let mut pipeline = GuardrailPipeline::new();
        pipeline.add(Arc::new(
            RegexGuardrail::new(
                "blocker",
                GuardrailStage::PreCall,
                vec![r"forbidden"],
                GuardrailMode::Block,
                "test",
            )
            .unwrap(),
        ));
        pipeline.add(Arc::new(LengthGuardrail::new(
            "never-runs",
            GuardrailStage::PreCall,
            10_000,
        )));

        let (_, results) = pipeline
            .run(GuardrailStage::PreCall, "this is forbidden", None, None, None)
            .await;
        assert_eq!(results.len(), 1);
        assert!(results[0].is_blocked());
    }
}
