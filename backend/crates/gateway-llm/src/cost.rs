use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Per-model pricing table (USD per 1M tokens)
#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input_per_1m: f64,
    pub output_per_1m: f64,
}

static PRICING_TABLE: Lazy<HashMap<&'static str, ModelPricing>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // ── OpenAI ──────────────────────────────────────────
    m.insert("gpt-4o", ModelPricing { input_per_1m: 2.50, output_per_1m: 10.0 });
    m.insert("gpt-4o-mini", ModelPricing { input_per_1m: 0.15, output_per_1m: 0.60 });
    m.insert("gpt-4-turbo", ModelPricing { input_per_1m: 10.0, output_per_1m: 30.0 });
    m.insert("gpt-3.5-turbo", ModelPricing { input_per_1m: 0.50, output_per_1m: 1.50 });
    m.insert("gpt-4.1", ModelPricing { input_per_1m: 2.0, output_per_1m: 8.0 });
    m.insert("gpt-4.1-mini", ModelPricing { input_per_1m: 0.40, output_per_1m: 1.60 });
    m.insert("gpt-4.1-nano", ModelPricing { input_per_1m: 0.10, output_per_1m: 0.40 });
    m.insert("o3", ModelPricing { input_per_1m: 2.0, output_per_1m: 8.0 });
    m.insert("o4-mini", ModelPricing { input_per_1m: 1.10, output_per_1m: 4.40 });

    // ── Anthropic ───────────────────────────────────────
    m.insert("claude-sonnet-4-20250514", ModelPricing { input_per_1m: 3.0, output_per_1m: 15.0 });
    m.insert("claude-opus-4-20250514", ModelPricing { input_per_1m: 15.0, output_per_1m: 75.0 });
    m.insert("claude-haiku-3-5-20241022", ModelPricing { input_per_1m: 0.80, output_per_1m: 4.0 });
    m.insert("claude-3-5-sonnet-20241022", ModelPricing { input_per_1m: 3.0, output_per_1m: 15.0 });
    m.insert("claude-3-haiku-20240307", ModelPricing { input_per_1m: 0.25, output_per_1m: 1.25 });
    m.insert("claude-3-opus-20240229", ModelPricing { input_per_1m: 15.0, output_per_1m: 75.0 });

    // ── Google ──────────────────────────────────────────
    m.insert("gemini-2.5-pro", ModelPricing { input_per_1m: 1.25, output_per_1m: 10.0 });
    m.insert("gemini-2.5-flash", ModelPricing { input_per_1m: 0.15, output_per_1m: 0.60 });
    m.insert("gemini-1.5-pro", ModelPricing { input_per_1m: 3.50, output_per_1m: 10.50 });
    m.insert("gemini-1.5-flash", ModelPricing { input_per_1m: 0.075, output_per_1m: 0.30 });

    // ── Qwen (Alibaba Cloud DashScope) ──────────────────
    m.insert("qwen-max", ModelPricing { input_per_1m: 2.0, output_per_1m: 6.0 });
    m.insert("qwen-plus", ModelPricing { input_per_1m: 0.50, output_per_1m: 1.50 });
    m.insert("qwen-turbo", ModelPricing { input_per_1m: 0.10, output_per_1m: 0.30 });
    m.insert("qwen-long", ModelPricing { input_per_1m: 0.50, output_per_1m: 2.0 });

    // ── xAI (Grok) ─────────────────────────────────────
    m.insert("grok-2", ModelPricing { input_per_1m: 2.0, output_per_1m: 10.0 });
    m.insert("grok-2-mini", ModelPricing { input_per_1m: 0.10, output_per_1m: 0.40 });
    m.insert("grok-3", ModelPricing { input_per_1m: 3.0, output_per_1m: 15.0 });
    m.insert("grok-3-mini", ModelPricing { input_per_1m: 0.30, output_per_1m: 0.50 });

    m
});

pub struct CostCalculator;

impl CostCalculator {
    /// Calculate cost in USD for the given token counts.
    pub fn calculate(model: &str, tokens_in: u64, tokens_out: u64) -> f64 {
        if let Some(pricing) = PRICING_TABLE.get(model) {
            let input_cost = (tokens_in as f64 / 1_000_000.0) * pricing.input_per_1m;
            let output_cost = (tokens_out as f64 / 1_000_000.0) * pricing.output_per_1m;
            input_cost + output_cost
        } else {
            // Unknown model: assume GPT-4o pricing as safe default
            let default = &ModelPricing { input_per_1m: 2.50, output_per_1m: 10.0 };
            (tokens_in as f64 / 1_000_000.0) * default.input_per_1m
                + (tokens_out as f64 / 1_000_000.0) * default.output_per_1m
        }
    }

    pub fn pricing_table() -> &'static HashMap<&'static str, ModelPricing> {
        &PRICING_TABLE
    }
}
