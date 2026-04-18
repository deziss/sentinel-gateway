//! Build a `GuardrailPipeline` from stored `GuardrailRule` rows.
//!
//! Each rule's `kind` + `config` is translated into a concrete [`Guardrail`]
//! implementation. Invalid configurations are skipped with a warning — the
//! gateway never fails to start because of one bad rule.

use gateway_db::models::guardrail_rule::GuardrailRule;
use gateway_policy::{
    Guardrail, GuardrailMode, GuardrailPipeline, GuardrailStage,
    JsonSchemaGuardrail, LengthGuardrail, RegexGuardrail,
};
use std::sync::Arc;

fn parse_stage(s: &str) -> GuardrailStage {
    match s {
        "pre_call" => GuardrailStage::PreCall,
        "post_call" => GuardrailStage::PostCall,
        "logging_only" => GuardrailStage::LoggingOnly,
        _ => GuardrailStage::PreCall,
    }
}

fn parse_mode(s: &str) -> GuardrailMode {
    match s {
        "block" => GuardrailMode::Block,
        "redact" => GuardrailMode::Redact,
        "flag" => GuardrailMode::Flag,
        _ => GuardrailMode::Flag,
    }
}

/// Build a pipeline from a list of stored rules. Invalid rules are logged and skipped.
pub fn build_pipeline(rules: &[GuardrailRule]) -> GuardrailPipeline {
    let mut pipeline = GuardrailPipeline::new();

    for rule in rules.iter().filter(|r| r.is_active) {
        let stage = parse_stage(&rule.stage);
        let mode = parse_mode(&rule.mode);

        let guard: Option<Arc<dyn Guardrail>> = match rule.kind.as_str() {
            "regex" => {
                let patterns: Vec<String> = rule
                    .config
                    .get("patterns")
                    .and_then(|p| p.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                if patterns.is_empty() {
                    tracing::warn!(rule = %rule.name, "regex rule has no patterns — skipping");
                    None
                } else {
                    let patterns_ref: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
                    match RegexGuardrail::new(
                        rule.name.clone(),
                        stage,
                        patterns_ref,
                        mode,
                        rule.category.clone(),
                    ) {
                        Ok(g) => Some(Arc::new(g)),
                        Err(e) => {
                            tracing::warn!(rule = %rule.name, error = %e, "regex compile failed");
                            None
                        }
                    }
                }
            }
            "length" => {
                let max_chars = rule
                    .config
                    .get("max_chars")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(100_000) as usize;
                Some(Arc::new(LengthGuardrail::new(
                    rule.name.clone(),
                    stage,
                    max_chars,
                )))
            }
            "json_schema" => {
                let schema = rule.config.get("schema").cloned().unwrap_or_default();
                Some(Arc::new(JsonSchemaGuardrail::new(rule.name.clone(), schema)))
            }
            "pii" => {
                // PII is implemented as a regex guardrail with predefined patterns
                // per type. This reuses the existing RegexGuardrail code path.
                let types: Vec<&str> = rule
                    .config
                    .get("types")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
                    .unwrap_or_else(|| vec!["email", "phone", "ssn", "credit_card"]);

                let patterns = expand_pii_patterns(&types);
                let patterns_ref: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();
                match RegexGuardrail::new(
                    rule.name.clone(),
                    stage,
                    patterns_ref,
                    mode,
                    rule.category.clone(),
                ) {
                    Ok(g) => Some(Arc::new(g)),
                    Err(e) => {
                        tracing::warn!(rule = %rule.name, error = %e, "pii pattern compile failed");
                        None
                    }
                }
            }
            other => {
                tracing::warn!(rule = %rule.name, kind = %other, "unknown guardrail kind — skipping");
                None
            }
        };

        if let Some(g) = guard {
            pipeline.add(g);
        }
    }

    pipeline
}

fn expand_pii_patterns(types: &[&str]) -> Vec<String> {
    let mut patterns = Vec::new();
    for t in types {
        match *t {
            "email" => patterns.push(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b".to_string()),
            "phone" => patterns.push(r"\b(?:\+?1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b".to_string()),
            "ssn" => patterns.push(r"\b\d{3}-\d{2}-\d{4}\b".to_string()),
            "credit_card" => patterns.push(r"\b(?:\d[ -]*?){13,19}\b".to_string()),
            "ipv4" => patterns.push(r"\b(?:\d{1,3}\.){3}\d{1,3}\b".to_string()),
            "aws_key" => patterns.push(r"\bAKIA[0-9A-Z]{16}\b".to_string()),
            _ => tracing::warn!(pii_type = %t, "unknown PII type"),
        }
    }
    patterns
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn mk_rule(kind: &str, stage: &str, mode: &str, config: serde_json::Value) -> GuardrailRule {
        GuardrailRule {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            name: format!("test-{kind}"),
            kind: kind.to_string(),
            stage: stage.to_string(),
            mode: mode.to_string(),
            category: "test".to_string(),
            config,
            priority: 100,
            is_active: true,
            created_by: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn build_pipeline_from_regex_rule() {
        let rule = mk_rule(
            "regex",
            "pre_call",
            "redact",
            serde_json::json!({"patterns": [r"\d{3}-\d{2}-\d{4}"]}),
        );
        let pipeline = build_pipeline(&[rule]);
        assert!(!pipeline.is_empty());
    }

    #[test]
    fn empty_patterns_skipped() {
        let rule = mk_rule("regex", "pre_call", "block", serde_json::json!({"patterns": []}));
        let pipeline = build_pipeline(&[rule]);
        assert!(pipeline.is_empty());
    }

    #[test]
    fn unknown_kind_skipped() {
        let rule = mk_rule("nonexistent", "pre_call", "block", serde_json::json!({}));
        let pipeline = build_pipeline(&[rule]);
        assert!(pipeline.is_empty());
    }

    #[test]
    fn inactive_rules_excluded() {
        let mut rule = mk_rule(
            "length",
            "pre_call",
            "block",
            serde_json::json!({"max_chars": 100}),
        );
        rule.is_active = false;
        let pipeline = build_pipeline(&[rule]);
        assert!(pipeline.is_empty());
    }
}
