use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::LlmError;
use crate::provider::{LlmProvider, ProviderType};

/// Metadata about a model discovered from a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: ProviderType,
    pub backend_id: Uuid,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub supports_vision: bool,
    pub supports_tools: bool,
    pub supports_streaming: bool,
}

/// Discovers and tracks available models across all registered providers.
/// Enterprise-only feature.
pub struct ModelFederation {
    http_client: reqwest::Client,
    known_models: DashMap<String, Vec<ModelInfo>>,
}

impl Default for ModelFederation {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelFederation {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
            known_models: DashMap::new(),
        }
    }

    /// Poll a provider's models endpoint to discover available models.
    pub async fn discover(&self, provider: &LlmProvider) -> Result<Vec<ModelInfo>, LlmError> {
        let url = provider.models_url();

        let mut req = self.http_client.get(&url);
        if let Some((header, value)) = provider.auth_header() {
            req = req.header(&header, &value);
        }

        let resp = req.send().await
            .map_err(|e| LlmError::ProviderError(format!("Model discovery failed for {}: {e}", provider.name)))?;

        if !resp.status().is_success() {
            return Err(LlmError::ProviderError(format!(
                "Model discovery returned {} for {}", resp.status(), provider.name
            )));
        }

        let body: serde_json::Value = resp.json().await
            .map_err(|e| LlmError::ProviderError(format!("Invalid models response: {e}")))?;

        let models = self.parse_models_response(&body, provider);

        // Store in known_models
        for model in &models {
            self.known_models
                .entry(model.id.clone())
                .or_default()
                .push(model.clone());
        }

        Ok(models)
    }

    fn parse_models_response(&self, body: &serde_json::Value, provider: &LlmProvider) -> Vec<ModelInfo> {
        let mut models = Vec::new();

        match provider.provider_type {
            ProviderType::Ollama => {
                // Ollama: { "models": [{ "name": "llama3", ... }] }
                if let Some(arr) = body.get("models").and_then(|m| m.as_array()) {
                    for m in arr {
                        if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                            models.push(ModelInfo {
                                id: name.to_string(),
                                provider: provider.provider_type.clone(),
                                backend_id: provider.id,
                                context_window: None,
                                max_output_tokens: None,
                                supports_vision: false,
                                supports_tools: false,
                                supports_streaming: true,
                            });
                        }
                    }
                }
            }
            _ => {
                // OpenAI-compatible: { "data": [{ "id": "gpt-4o", ... }] }
                if let Some(arr) = body.get("data").and_then(|d| d.as_array()) {
                    for m in arr {
                        if let Some(id) = m.get("id").and_then(|i| i.as_str()) {
                            models.push(ModelInfo {
                                id: id.to_string(),
                                provider: provider.provider_type.clone(),
                                backend_id: provider.id,
                                context_window: None,
                                max_output_tokens: None,
                                supports_vision: false,
                                supports_tools: false,
                                supports_streaming: true,
                            });
                        }
                    }
                }
            }
        }

        models
    }

    /// Get all known models across all providers.
    pub fn list_all(&self) -> Vec<ModelInfo> {
        self.known_models
            .iter()
            .flat_map(|entry| entry.value().clone())
            .collect()
    }

    /// Check if a model is available on any provider.
    pub fn is_available(&self, model: &str) -> bool {
        self.known_models.contains_key(model)
    }

    /// Clear all cached model info.
    pub fn clear(&self) {
        self.known_models.clear();
    }
}
