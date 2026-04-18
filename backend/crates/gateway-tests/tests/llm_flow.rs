//! Integration tests for LLM features: adapters, token counting, cost,
//! semantic cache, PII detection, intelligent routing.

use gateway_llm::{
    adapter::ProviderAdapter,
    cache::SemanticCache,
    cost::CostCalculator,
    pii,
    provider::{LlmProvider, ProviderType},
    router::LlmRouter,
    smart_router,
    token_counter::TokenCounter,
};
use serde_json::json;
use uuid::Uuid;

fn sample_provider() -> LlmProvider {
    LlmProvider {
        id: Uuid::new_v4(),
        name: "test-openai".to_string(),
        provider_type: ProviderType::OpenAi,
        endpoint: "https://api.openai.com/v1".to_string(),
        api_key: Some("sk-test".to_string()),
        models: vec!["gpt-4o".to_string()],
        priority: 0,
        weight: 1,
    }
}

// ── Provider routing ──────────────────────────────────────────────────────

#[test]
fn router_selects_registered_provider() {
    let router = LlmRouter::new();
    router.register("gpt-4o", sample_provider());

    let selected = router.select("gpt-4o").unwrap();
    assert_eq!(selected.provider_type, ProviderType::OpenAi);
}

#[test]
fn router_falls_back_to_wildcard() {
    let router = LlmRouter::new();
    router.register("*", sample_provider());

    let selected = router.select("any-model").unwrap();
    assert_eq!(selected.name, "test-openai");
}

#[test]
fn router_resolves_aliases() {
    let router = LlmRouter::new();
    router.register("gpt-4o", sample_provider());
    router.set_alias("gpt-4", "gpt-4o");

    let selected = router.select("gpt-4").unwrap();
    assert_eq!(selected.name, "test-openai");
}

#[test]
fn router_errors_on_unknown_model() {
    let router = LlmRouter::new();
    assert!(router.select("unknown-model").is_err());
}

// ── Provider URLs ─────────────────────────────────────────────────────────

#[test]
fn provider_chat_url_by_type() {
    let mut p = sample_provider();

    p.provider_type = ProviderType::OpenAi;
    assert_eq!(p.chat_url(), "https://api.openai.com/v1/chat/completions");

    p.provider_type = ProviderType::Anthropic;
    assert_eq!(p.chat_url(), "https://api.openai.com/v1/messages");

    p.provider_type = ProviderType::Ollama;
    assert_eq!(p.chat_url(), "https://api.openai.com/v1/api/chat");
}

#[test]
fn provider_auth_header_anthropic_uses_x_api_key() {
    let mut p = sample_provider();
    p.provider_type = ProviderType::Anthropic;

    let (header, value) = p.auth_header().unwrap();
    assert_eq!(header, "x-api-key");
    assert_eq!(value, "sk-test");
}

#[test]
fn provider_auth_header_openai_uses_bearer() {
    let p = sample_provider();
    let (header, value) = p.auth_header().unwrap();
    assert_eq!(header, "Authorization");
    assert_eq!(value, "Bearer sk-test");
}

// ── Token Counter ─────────────────────────────────────────────────────────

#[test]
fn token_counter_counts_simple_message() {
    let count = TokenCounter::count("Hello, world!");
    assert!(count > 0);
    assert!(count < 10);
}

#[test]
fn token_counter_counts_chat_messages() {
    let messages = vec![
        json!({"role": "system", "content": "You are helpful."}),
        json!({"role": "user", "content": "Hello"}),
    ];
    let count = TokenCounter::count_messages(&messages);
    assert!(count > 5);
}

// ── Cost Calculator ───────────────────────────────────────────────────────

#[test]
fn cost_calculator_known_model() {
    let cost = CostCalculator::calculate("gpt-4o", 1_000_000, 1_000_000);
    // gpt-4o: $2.50/1M input + $10/1M output = $12.50
    assert!((cost - 12.5).abs() < 0.01, "expected ~12.50, got {cost}");
}

#[test]
fn cost_calculator_unknown_model_uses_default() {
    let cost = CostCalculator::calculate("some-unknown-model", 1000, 1000);
    assert!(cost > 0.0);
}

// ── Request Adapter (OpenAI -> Anthropic) ────────────────────────────────

#[test]
fn adapter_anthropic_extracts_system_prompt() {
    let openai_req = json!({
        "model": "claude-sonnet-4-20250514",
        "messages": [
            {"role": "system", "content": "Be concise."},
            {"role": "user", "content": "Hi"}
        ],
        "max_tokens": 100
    });

    let anthropic = ProviderAdapter::to_anthropic(&openai_req).unwrap();
    assert_eq!(anthropic["system"], "Be concise.");
    // System message must be removed from messages array
    assert_eq!(anthropic["messages"].as_array().unwrap().len(), 1);
    assert_eq!(anthropic["messages"][0]["role"], "user");
}

#[test]
fn adapter_anthropic_response_converts_to_openai_format() {
    let anthropic_resp = json!({
        "content": [{"type": "text", "text": "Hello!"}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 5}
    });

    let openai = ProviderAdapter::anthropic_response_to_openai(&anthropic_resp, "claude-sonnet-4");
    assert_eq!(openai["choices"][0]["message"]["content"], "Hello!");
    assert_eq!(openai["choices"][0]["finish_reason"], "stop");
    assert_eq!(openai["usage"]["prompt_tokens"], 10);
    assert_eq!(openai["usage"]["completion_tokens"], 5);
    assert_eq!(openai["usage"]["total_tokens"], 15);
}

// ── Semantic Cache ────────────────────────────────────────────────────────

#[test]
fn semantic_cache_hits_on_identical_request() {
    let cache = SemanticCache::new(300, 100);
    let req = json!({
        "model": "gpt-4o",
        "messages": [{"role": "user", "content": "Hello"}],
        "temperature": 0.0
    });

    let key = SemanticCache::cache_key(&req);
    assert!(cache.get(&key).is_none());

    cache.put(key.clone(), gateway_llm::cache::CachedResponse {
        response: json!({"message": "cached"}),
        model: "gpt-4o".to_string(),
        tokens_input: 5,
        tokens_output: 2,
        cost_usd: 0.0,
    });

    let hit = cache.get(&key).unwrap();
    assert_eq!(hit.model, "gpt-4o");
}

#[test]
fn semantic_cache_skips_streaming_requests() {
    let req = json!({"stream": true, "messages": []});
    assert!(!SemanticCache::should_cache(&req));
}

#[test]
fn semantic_cache_skips_high_temperature() {
    let req = json!({"temperature": 0.9, "messages": []});
    assert!(!SemanticCache::should_cache(&req));
}

#[test]
fn semantic_cache_includes_low_temperature() {
    let req = json!({"temperature": 0.1, "messages": []});
    assert!(SemanticCache::should_cache(&req));
}

// ── PII Detection ─────────────────────────────────────────────────────────

#[test]
fn pii_detects_email() {
    let matches = pii::detect("Contact user@example.com for details");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].pattern_type, "email");
}

#[test]
fn pii_detects_multiple_types() {
    let text = "Email alice@example.com or SSN 123-45-6789";
    let matches = pii::detect(text);
    assert!(matches.len() >= 2);
}

#[test]
fn pii_redact_replaces_with_placeholders() {
    let redacted = pii::redact("Contact john@example.com");
    assert!(redacted.contains("[REDACTED:email]"));
    assert!(!redacted.contains("john@example.com"));
}

#[test]
fn pii_clean_text_has_no_matches() {
    let matches = pii::detect("Just a regular sentence with no sensitive info.");
    assert!(matches.is_empty());
}

// ── Intelligent Routing ───────────────────────────────────────────────────

#[test]
fn smart_router_classifies_simple() {
    let req = json!({"messages": [{"role": "user", "content": "hi"}]});
    let c = smart_router::analyze_complexity(&req);
    assert_eq!(c, smart_router::PromptComplexity::Simple);
}

#[test]
fn smart_router_classifies_complex_with_code() {
    let req = json!({
        "messages": [{"role": "user", "content": "Explain this code: ```fn foo() {}```"}]
    });
    let c = smart_router::analyze_complexity(&req);
    assert_eq!(c, smart_router::PromptComplexity::Complex);
}

#[test]
fn smart_router_auto_selects_for_simple() {
    let req = json!({"model": "auto", "messages": [{"role": "user", "content": "hi"}]});
    let available = vec!["gpt-4o-mini".to_string(), "gpt-4.1".to_string()];

    let selected = smart_router::select_model(&req, &available).unwrap();
    assert_eq!(selected, "gpt-4o-mini", "simple prompt should use cheap model");
}

#[test]
fn smart_router_respects_explicit_model() {
    let req = json!({"model": "gpt-4.1", "messages": []});
    let available = vec!["gpt-4o-mini".to_string(), "gpt-4.1".to_string()];

    let selected = smart_router::select_model(&req, &available).unwrap();
    assert_eq!(selected, "gpt-4.1", "explicit model must be respected");
}
