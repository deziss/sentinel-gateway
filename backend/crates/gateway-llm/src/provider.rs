use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    // Original providers
    OpenAi,
    Anthropic,
    GoogleVertex,
    AwsBedrock,
    Ollama,
    Vllm,
    OpenAiCompatible,
    Qwen,
    Xai,
    Zai,
    // New OpenAI-compatible providers (P1 catalog expansion)
    Mistral,
    Cohere,
    DeepSeek,
    Groq,
    Together,
    Perplexity,
    Fireworks,
}

impl ProviderType {
    /// Default endpoint for this provider (when user doesn't override).
    pub fn default_endpoint(&self) -> Option<&'static str> {
        match self {
            Self::OpenAi => Some("https://api.openai.com/v1"),
            Self::Anthropic => Some("https://api.anthropic.com/v1"),
            Self::Mistral => Some("https://api.mistral.ai/v1"),
            Self::Cohere => Some("https://api.cohere.com/compatibility/v1"),
            Self::DeepSeek => Some("https://api.deepseek.com/v1"),
            Self::Groq => Some("https://api.groq.com/openai/v1"),
            Self::Together => Some("https://api.together.xyz/v1"),
            Self::Perplexity => Some("https://api.perplexity.ai"),
            Self::Fireworks => Some("https://api.fireworks.ai/inference/v1"),
            Self::Xai => Some("https://api.x.ai/v1"),
            _ => None,
        }
    }

    /// Whether this provider uses the OpenAI-compatible request/response shape.
    /// All new P1 providers are OpenAI-compatible (no adapter translation needed).
    pub fn is_openai_compatible(&self) -> bool {
        matches!(
            self,
            Self::OpenAi
                | Self::OpenAiCompatible
                | Self::Vllm
                | Self::Xai
                | Self::Qwen
                | Self::Zai
                | Self::AwsBedrock
                | Self::Mistral
                | Self::Cohere
                | Self::DeepSeek
                | Self::Groq
                | Self::Together
                | Self::Perplexity
                | Self::Fireworks
        )
    }
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ProviderType::OpenAi => "openai",
            ProviderType::Anthropic => "anthropic",
            ProviderType::GoogleVertex => "google_vertex",
            ProviderType::AwsBedrock => "aws_bedrock",
            ProviderType::Ollama => "ollama",
            ProviderType::Vllm => "vllm",
            ProviderType::OpenAiCompatible => "openai_compatible",
            ProviderType::Qwen => "qwen",
            ProviderType::Xai => "xai",
            ProviderType::Zai => "zai",
            ProviderType::Mistral => "mistral",
            ProviderType::Cohere => "cohere",
            ProviderType::DeepSeek => "deepseek",
            ProviderType::Groq => "groq",
            ProviderType::Together => "together",
            ProviderType::Perplexity => "perplexity",
            ProviderType::Fireworks => "fireworks",
        };
        write!(f, "{s}")
    }
}

/// An LLM provider backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProvider {
    pub id: uuid::Uuid,
    pub name: String,
    pub provider_type: ProviderType,
    pub endpoint: String,
    pub api_key: Option<String>,
    pub models: Vec<String>,
    pub priority: i32,
    pub weight: i32,
}

impl LlmProvider {
    /// Returns the chat completions URL for this provider.
    pub fn chat_url(&self) -> String {
        let base = self.endpoint.trim_end_matches('/');
        match self.provider_type {
            ProviderType::Ollama => format!("{base}/api/chat"),
            ProviderType::Anthropic => format!("{base}/messages"),
            // All OpenAI-compatible providers (including new P1 catalog) use /chat/completions
            _ => format!("{base}/chat/completions"),
        }
    }

    /// Returns the embeddings URL for this provider.
    pub fn embeddings_url(&self) -> String {
        let base = self.endpoint.trim_end_matches('/');
        match self.provider_type {
            ProviderType::Ollama => format!("{base}/api/embeddings"),
            _ => format!("{base}/embeddings"),
        }
    }

    /// Returns the models list URL for this provider.
    pub fn models_url(&self) -> String {
        let base = self.endpoint.trim_end_matches('/');
        match self.provider_type {
            ProviderType::Ollama => format!("{base}/api/tags"),
            ProviderType::Anthropic => format!("{base}/models"),
            _ => format!("{base}/models"),
        }
    }

    /// Returns the image generation URL (OpenAI-compatible only — Anthropic/Gemini not supported).
    pub fn images_generations_url(&self) -> String {
        let base = self.endpoint.trim_end_matches('/');
        format!("{base}/images/generations")
    }

    /// Returns the image edit URL (OpenAI-compatible).
    pub fn images_edits_url(&self) -> String {
        let base = self.endpoint.trim_end_matches('/');
        format!("{base}/images/edits")
    }

    /// Returns the audio transcription URL (OpenAI-compatible — whisper).
    pub fn audio_transcriptions_url(&self) -> String {
        let base = self.endpoint.trim_end_matches('/');
        format!("{base}/audio/transcriptions")
    }

    /// Returns the audio speech synthesis URL (OpenAI-compatible — TTS).
    pub fn audio_speech_url(&self) -> String {
        let base = self.endpoint.trim_end_matches('/');
        format!("{base}/audio/speech")
    }

    /// Whether this provider supports OpenAI-compatible multimodal endpoints.
    /// (Most providers don't host image/audio models; OpenAI + Together + Fireworks do.)
    pub fn supports_multimodal(&self) -> bool {
        matches!(
            self.provider_type,
            ProviderType::OpenAi
                | ProviderType::OpenAiCompatible
                | ProviderType::Vllm
                | ProviderType::Xai
                | ProviderType::Qwen
                | ProviderType::Zai
                | ProviderType::Together
                | ProviderType::Fireworks
        )
    }

    pub fn auth_header(&self) -> Option<(String, String)> {
        self.api_key.as_ref().map(|key| {
            match self.provider_type {
                ProviderType::Anthropic => ("x-api-key".to_string(), key.clone()),
                _ => ("Authorization".to_string(), format!("Bearer {key}")),
            }
        })
    }
}
