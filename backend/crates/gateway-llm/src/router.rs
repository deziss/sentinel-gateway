use dashmap::DashMap;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::{error::LlmError, provider::LlmProvider};

/// Routes LLM requests to the correct provider based on model name.
/// Supports priority-based fallback, weighted distribution, and model aliasing.
pub struct LlmRouter {
    /// model alias → list of providers (sorted by priority)
    routes: DashMap<String, Vec<LlmProvider>>,
    /// round-robin counters per model
    counters: DashMap<String, Arc<Mutex<usize>>>,
    /// model aliases: "gpt-4" → "gpt-4o"
    aliases: DashMap<String, String>,
    /// Fallback chains: model → [fallback_model_1, fallback_model_2, ...]
    /// On provider failure (5xx/429/timeout), try the next model in the chain.
    fallback_chains: DashMap<String, Vec<String>>,
}

impl LlmRouter {
    pub fn new() -> Self {
        Self {
            routes: DashMap::new(),
            counters: DashMap::new(),
            aliases: DashMap::new(),
            fallback_chains: DashMap::new(),
        }
    }

    pub fn register(&self, model: impl Into<String>, provider: LlmProvider) {
        let model = model.into();
        let mut entry = self.routes.entry(model.clone()).or_insert_with(Vec::new);
        entry.push(provider);
        entry.sort_by_key(|p| p.priority);
    }

    /// Set a model alias: requests for `alias` will be resolved to `target`.
    pub fn set_alias(&self, alias: impl Into<String>, target: impl Into<String>) {
        self.aliases.insert(alias.into(), target.into());
    }

    /// Resolve an alias to the actual model name. Returns the input if no alias.
    pub fn resolve_alias(&self, model: &str) -> String {
        self.aliases
            .get(model)
            .map(|v| v.clone())
            .unwrap_or_else(|| model.to_string())
    }

    /// Configure a fallback chain for a model.
    /// Example: `set_fallback("gpt-4o", vec!["claude-sonnet-4", "gemini-2.5-pro"])`
    /// If gpt-4o fails, try claude-sonnet-4; if that fails, try gemini-2.5-pro.
    pub fn set_fallback(&self, primary: impl Into<String>, fallbacks: Vec<String>) {
        self.fallback_chains.insert(primary.into(), fallbacks);
    }

    /// Get the fallback chain for a model (empty if none configured).
    pub fn get_fallbacks(&self, model: &str) -> Vec<String> {
        self.fallback_chains
            .get(model)
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Resolve a model + its fallback chain into an ordered list of models to try.
    /// The primary is first, followed by each fallback in order.
    /// Aliases are resolved at this stage.
    pub fn resolution_chain(&self, model: &str) -> Vec<String> {
        let primary = self.resolve_alias(model);
        let mut chain = vec![primary.clone()];
        for fb in self.get_fallbacks(&primary) {
            chain.push(self.resolve_alias(&fb));
        }
        chain
    }

    /// Select a provider for the given model (with round-robin among equal-priority providers).
    /// Resolves aliases before lookup. Does NOT apply fallbacks — use `select_with_fallback`
    /// or iterate `resolution_chain` for that.
    pub fn select(&self, model: &str) -> Result<LlmProvider, LlmError> {
        let resolved = self.resolve_alias(model);

        // Try exact match, then wildcard "*"
        let providers = self
            .routes
            .get(&resolved)
            .or_else(|| self.routes.get("*"))
            .ok_or_else(|| LlmError::NoBackend(resolved.clone()))?;

        if providers.is_empty() {
            return Err(LlmError::NoBackend(resolved));
        }

        let counter = self
            .counters
            .entry(resolved)
            .or_insert_with(|| Arc::new(Mutex::new(0)));

        let mut c = counter.lock();
        let idx = *c % providers.len();
        *c = c.wrapping_add(1);

        Ok(providers[idx].clone())
    }

    /// Iterate through the resolution chain and return the first provider that can be selected.
    /// The returned tuple is `(selected_model, provider)` — `selected_model` may differ from
    /// the input if a fallback was used.
    ///
    /// This does NOT execute the request — the caller is responsible for calling the provider
    /// and retrying with the next element of `resolution_chain` on failure.
    pub fn select_with_chain(&self, model: &str) -> Vec<(String, LlmProvider)> {
        self.resolution_chain(model)
            .into_iter()
            .filter_map(|m| {
                self.select(&m).ok().map(|p| (m, p))
            })
            .collect()
    }

    pub fn list_models(&self) -> Vec<String> {
        self.routes.iter().map(|e| e.key().clone()).collect()
    }

    pub fn list_aliases(&self) -> Vec<(String, String)> {
        self.aliases.iter().map(|e| (e.key().clone(), e.value().clone())).collect()
    }

    pub fn list_fallback_chains(&self) -> Vec<(String, Vec<String>)> {
        self.fallback_chains
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect()
    }
}

impl Default for LlmRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{LlmProvider, ProviderType};

    fn make_provider(name: &str) -> LlmProvider {
        LlmProvider {
            id: uuid::Uuid::new_v4(),
            name: name.to_string(),
            provider_type: ProviderType::OpenAi,
            endpoint: format!("https://{name}/v1"),
            api_key: None,
            models: vec![],
            priority: 10,
            weight: 1,
        }
    }

    #[test]
    fn fallback_chain_ordering() {
        let router = LlmRouter::new();
        router.register("gpt-4o", make_provider("openai"));
        router.register("claude-sonnet-4", make_provider("anthropic"));
        router.register("gemini-2.5-pro", make_provider("google"));

        router.set_fallback("gpt-4o", vec![
            "claude-sonnet-4".into(),
            "gemini-2.5-pro".into(),
        ]);

        let chain = router.resolution_chain("gpt-4o");
        assert_eq!(chain, vec!["gpt-4o", "claude-sonnet-4", "gemini-2.5-pro"]);
    }

    #[test]
    fn resolution_chain_respects_aliases() {
        let router = LlmRouter::new();
        router.register("gpt-4o-2024-11-20", make_provider("openai"));
        router.register("claude-sonnet-4-20250514", make_provider("anthropic"));

        router.set_alias("gpt-4o", "gpt-4o-2024-11-20");
        router.set_alias("claude", "claude-sonnet-4-20250514");
        router.set_fallback("gpt-4o-2024-11-20", vec!["claude".into()]);

        let chain = router.resolution_chain("gpt-4o");
        assert_eq!(chain, vec!["gpt-4o-2024-11-20", "claude-sonnet-4-20250514"]);
    }

    #[test]
    fn no_fallback_returns_single_element_chain() {
        let router = LlmRouter::new();
        router.register("gpt-4o", make_provider("openai"));
        let chain = router.resolution_chain("gpt-4o");
        assert_eq!(chain, vec!["gpt-4o"]);
    }
}
