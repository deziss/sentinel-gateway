use crate::token_counter::TokenCounter;

/// Estimated complexity of a prompt.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PromptComplexity {
    /// Short, simple query (< 100 tokens, no code, no system prompt)
    Simple,
    /// Moderate query (100-500 tokens, or has system prompt)
    Moderate,
    /// Complex query (> 500 tokens, code blocks, multi-turn, or reasoning needed)
    Complex,
}

/// Analyze a chat completion request and estimate its complexity.
pub fn analyze_complexity(request: &serde_json::Value) -> PromptComplexity {
    let messages = match request.get("messages").and_then(|m| m.as_array()) {
        Some(m) => m,
        None => return PromptComplexity::Simple,
    };

    let total_tokens = TokenCounter::count_messages(messages) as usize;
    let message_count = messages.len();
    let has_system_prompt = messages.iter().any(|m| {
        m.get("role").and_then(|r| r.as_str()) == Some("system")
    });

    // Check for code blocks or technical content
    let has_code = messages.iter().any(|m| {
        m.get("content")
            .and_then(|c| c.as_str())
            .map(|content| content.contains("```") || content.contains("def ") || content.contains("fn ") || content.contains("function "))
            .unwrap_or(false)
    });

    // Check for reasoning keywords
    let has_reasoning = messages.iter().any(|m| {
        m.get("content")
            .and_then(|c| c.as_str())
            .map(|content| {
                let lower = content.to_lowercase();
                lower.contains("explain") || lower.contains("analyze") || lower.contains("compare")
                    || lower.contains("step by step") || lower.contains("reasoning")
            })
            .unwrap_or(false)
    });

    if total_tokens > 500 || has_code || has_reasoning || message_count > 5 {
        PromptComplexity::Complex
    } else if total_tokens > 100 || has_system_prompt || message_count > 2 {
        PromptComplexity::Moderate
    } else {
        PromptComplexity::Simple
    }
}

/// Model tier recommendation based on complexity.
#[derive(Debug, Clone)]
pub struct ModelRecommendation {
    pub preferred_models: Vec<&'static str>,
    pub complexity: PromptComplexity,
    pub reason: &'static str,
}

/// Recommend model tiers based on prompt complexity.
pub fn recommend_models(complexity: &PromptComplexity) -> ModelRecommendation {
    match complexity {
        PromptComplexity::Simple => ModelRecommendation {
            preferred_models: vec![
                "gpt-4o-mini", "gpt-4.1-nano", "claude-haiku-3-5-20241022",
                "gemini-2.5-flash", "grok-3-mini", "qwen-turbo",
            ],
            complexity: PromptComplexity::Simple,
            reason: "Simple query — using fast, cost-effective model",
        },
        PromptComplexity::Moderate => ModelRecommendation {
            preferred_models: vec![
                "gpt-4o", "gpt-4.1-mini", "claude-sonnet-4-20250514",
                "gemini-2.5-pro", "grok-2", "qwen-plus",
            ],
            complexity: PromptComplexity::Moderate,
            reason: "Moderate query — using balanced model",
        },
        PromptComplexity::Complex => ModelRecommendation {
            preferred_models: vec![
                "gpt-4.1", "o3", "claude-opus-4-20250514",
                "grok-3", "qwen-max",
            ],
            complexity: PromptComplexity::Complex,
            reason: "Complex query — using high-capability model",
        },
    }
}

/// Given a request and available model list, select the best model.
/// If the requested model is "auto" or not specified, uses intelligent routing.
/// Otherwise, respects the explicit model choice.
pub fn select_model(
    request: &serde_json::Value,
    available_models: &[String],
) -> Option<String> {
    let requested = request.get("model").and_then(|m| m.as_str()).unwrap_or("auto");

    // If explicit model specified (not "auto"), use it
    if requested != "auto" {
        return Some(requested.to_string());
    }

    // Intelligent routing
    let complexity = analyze_complexity(request);
    let recommendation = recommend_models(&complexity);

    // Find first available preferred model
    for preferred in recommendation.preferred_models {
        if available_models.iter().any(|m| m == preferred) {
            return Some(preferred.to_string());
        }
    }

    // Fallback to first available model
    available_models.first().cloned()
}
