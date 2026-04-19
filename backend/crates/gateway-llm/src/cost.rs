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

    /// Calculate cost with an optional tenant-specific override.
    ///
    /// Override fields are applied first (input/output), then `markup` is
    /// multiplied on top. If no override is supplied, uses the default table.
    pub fn calculate_with_override(
        model: &str,
        tokens_in: u64,
        tokens_out: u64,
        override_input_per_1m: Option<f64>,
        override_output_per_1m: Option<f64>,
        markup: f64,
    ) -> f64 {
        let default = PRICING_TABLE.get(model).cloned().unwrap_or(ModelPricing {
            input_per_1m: 2.50,
            output_per_1m: 10.0,
        });
        let input_price = override_input_per_1m.unwrap_or(default.input_per_1m);
        let output_price = override_output_per_1m.unwrap_or(default.output_per_1m);
        let cost = (tokens_in as f64 / 1_000_000.0) * input_price
            + (tokens_out as f64 / 1_000_000.0) * output_price;
        cost * markup.max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected ≈ {b}, got {a}");
    }

    #[test]
    fn override_with_no_overrides_matches_default_table_and_markup_1() {
        // 1M in + 1M out on a model, markup 1.0 → cost = input_price + output_price
        let c = CostCalculator::calculate_with_override(
            "gpt-4o", 1_000_000, 1_000_000, None, None, 1.0,
        );
        let table = CostCalculator::pricing_table()
            .get("gpt-4o")
            .cloned()
            .expect("gpt-4o present in default pricing table");
        approx(c, table.input_per_1m + table.output_per_1m);
    }

    #[test]
    fn override_input_price_is_applied() {
        // Pure-input: 500k tokens @ $10/1M = $5
        let c = CostCalculator::calculate_with_override(
            "gpt-4o", 500_000, 0, Some(10.0), None, 1.0,
        );
        approx(c, 5.0);
    }

    #[test]
    fn override_output_price_is_applied() {
        // Pure-output: 500k tokens @ $20/1M = $10
        let c = CostCalculator::calculate_with_override(
            "gpt-4o", 0, 500_000, None, Some(20.0), 1.0,
        );
        approx(c, 10.0);
    }

    #[test]
    fn markup_multiplier_scales_final_cost() {
        // Both overridden to $1/1M → 1M in + 1M out = $2, × 2.0 = $4
        let c = CostCalculator::calculate_with_override(
            "gpt-4o", 1_000_000, 1_000_000, Some(1.0), Some(1.0), 2.0,
        );
        approx(c, 4.0);
    }

    #[test]
    fn negative_markup_is_clamped_to_zero() {
        let c = CostCalculator::calculate_with_override(
            "gpt-4o", 1_000_000, 1_000_000, Some(1.0), Some(1.0), -5.0,
        );
        approx(c, 0.0);
    }

    #[test]
    fn zero_tokens_gives_zero_cost() {
        let c = CostCalculator::calculate_with_override(
            "gpt-4o", 0, 0, None, None, 1.0,
        );
        approx(c, 0.0);
    }

    #[test]
    fn unknown_model_uses_fallback_pricing() {
        // Fallback is 2.50 / 10.0 per 1M
        let c = CostCalculator::calculate_with_override(
            "unknown-model-xyz", 1_000_000, 1_000_000, None, None, 1.0,
        );
        approx(c, 2.50 + 10.0);
    }

    #[test]
    fn partial_override_input_only_falls_back_to_default_output() {
        let c = CostCalculator::calculate_with_override(
            "gpt-4o", 1_000_000, 1_000_000, Some(0.0), None, 1.0,
        );
        let table = CostCalculator::pricing_table().get("gpt-4o").cloned().unwrap();
        approx(c, 0.0 + table.output_per_1m);
    }
}
