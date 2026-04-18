use serde_json::Value;
use crate::error::LlmError;
use crate::provider::ProviderType;

/// Parsed token usage for a single completion. Captures the modern fields
/// needed for accurate cost accounting across OpenAI / Anthropic / Gemini.
///
/// - `prompt_tokens` is the **billable** input count (Anthropic reports this net of cache).
/// - `cached_tokens` and `cache_creation_tokens` are priced at different rates than fresh input.
/// - `reasoning_tokens` (OpenAI o-series, Claude extended thinking) are output tokens but
///   often billed differently and worth tracking separately for observability.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub cached_tokens: u64,
    pub cache_creation_tokens: u64,
    pub reasoning_tokens: u64,
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        self.prompt_tokens + self.completion_tokens
    }

    /// Extract usage from an OpenAI-format response (after adapter conversion).
    pub fn from_openai_response(resp: &Value) -> Self {
        let usage = match resp.get("usage") {
            Some(u) => u,
            None => return Self::default(),
        };
        let prompt_tokens = usage.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let completion_tokens = usage.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

        let details = usage.get("prompt_tokens_details");
        let cached_tokens = details
            .and_then(|d| d.get("cached_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let cache_creation_tokens = details
            .and_then(|d| d.get("cache_creation_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let reasoning_tokens = usage
            .get("completion_tokens_details")
            .and_then(|d| d.get("reasoning_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Self {
            prompt_tokens,
            completion_tokens,
            cached_tokens,
            cache_creation_tokens,
            reasoning_tokens,
        }
    }
}

/// Adapts OpenAI-format requests/responses to and from other provider formats.
pub struct ProviderAdapter;

impl ProviderAdapter {
    // ── Request adapters (OpenAI → Provider) ───────────────────────────────

    /// Dispatch: adapt an OpenAI-format request to the target provider's format.
    /// OpenAI-compatible providers (OpenAI, xAI, Qwen, ZAI, Vllm) pass through unchanged.
    pub fn adapt_request(provider_type: &ProviderType, openai_req: &Value) -> Result<Value, LlmError> {
        match provider_type {
            ProviderType::Anthropic => Self::to_anthropic(openai_req),
            ProviderType::GoogleVertex => Self::to_gemini(openai_req),
            ProviderType::Ollama => Self::to_ollama(openai_req),
            // OpenAI-compatible providers: pass through
            _ => Ok(openai_req.clone()),
        }
    }

    /// Dispatch: adapt a provider response back to OpenAI format.
    pub fn adapt_response(provider_type: &ProviderType, resp: &Value, model: &str) -> Value {
        match provider_type {
            ProviderType::Anthropic => Self::anthropic_response_to_openai(resp, model),
            ProviderType::GoogleVertex => Self::gemini_response_to_openai(resp, model),
            ProviderType::Ollama => Self::ollama_response_to_openai(resp),
            // OpenAI-compatible: pass through
            _ => resp.clone(),
        }
    }

    // ── OpenAI → Anthropic ─────────────────────────────────────────────────

    pub fn to_anthropic(openai_req: &Value) -> Result<Value, LlmError> {
        let messages = openai_req.get("messages").and_then(|m| m.as_array())
            .ok_or_else(|| LlmError::Internal("Missing messages in request".into()))?;

        let mut anthropic_messages = Vec::new();
        let mut system_blocks: Vec<Value> = Vec::new();

        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            // Preserve cache_control if the client already added it (Anthropic style)
            let cache_control = msg.get("cache_control").cloned();

            // Content can be a string or an array of content blocks (vision / cache markers)
            let content_value = match msg.get("content") {
                Some(Value::String(s)) => {
                    if let Some(cc) = cache_control.clone() {
                        // Promote to block form so we can attach cache_control
                        Value::Array(vec![serde_json::json!({
                            "type": "text",
                            "text": s,
                            "cache_control": cc,
                        })])
                    } else {
                        Value::String(s.clone())
                    }
                }
                Some(Value::Array(blocks)) => Value::Array(blocks.clone()),
                _ => Value::String(String::new()),
            };

            match role {
                "system" => {
                    // System messages become Anthropic system blocks
                    match content_value {
                        Value::String(s) => {
                            let mut block = serde_json::json!({ "type": "text", "text": s });
                            if let Some(cc) = cache_control {
                                block.as_object_mut().unwrap().insert("cache_control".into(), cc);
                            }
                            system_blocks.push(block);
                        }
                        Value::Array(arr) => system_blocks.extend(arr),
                        _ => {}
                    }
                }
                "user" | "assistant" => {
                    anthropic_messages.push(serde_json::json!({
                        "role": role,
                        "content": content_value,
                    }));
                }
                _ => {}
            }
        }

        let mut payload = serde_json::json!({
            "model": openai_req.get("model").unwrap_or(&Value::String("claude-sonnet-4-20250514".into())),
            "messages": anthropic_messages,
            "max_tokens": openai_req.get("max_tokens").unwrap_or(&Value::Number(1024.into())),
        });

        // Attach system blocks (Anthropic supports array of blocks with cache_control)
        if !system_blocks.is_empty() {
            if system_blocks.len() == 1 && system_blocks[0].get("cache_control").is_none() {
                // Simple case: single system block with no cache → use string form
                if let Some(text) = system_blocks[0].get("text").and_then(|v| v.as_str()) {
                    payload.as_object_mut().unwrap().insert("system".into(), Value::String(text.into()));
                }
            } else {
                payload.as_object_mut().unwrap().insert("system".into(), Value::Array(system_blocks));
            }
        }

        if let Some(temp) = openai_req.get("temperature") {
            payload.as_object_mut().unwrap().insert("temperature".into(), temp.clone());
        }
        if let Some(stream) = openai_req.get("stream") {
            payload.as_object_mut().unwrap().insert("stream".into(), stream.clone());
        }
        if let Some(top_p) = openai_req.get("top_p") {
            payload.as_object_mut().unwrap().insert("top_p".into(), top_p.clone());
        }
        // Pass through tools for tool use
        if let Some(tools) = openai_req.get("tools") {
            payload.as_object_mut().unwrap().insert("tools".into(), tools.clone());
        }

        Ok(payload)
    }

    // ── OpenAI → Gemini ────────────────────────────────────────────────────

    pub fn to_gemini(openai_req: &Value) -> Result<Value, LlmError> {
        let messages = openai_req.get("messages").and_then(|m| m.as_array())
            .ok_or_else(|| LlmError::Internal("Missing messages in request".into()))?;

        let mut contents = Vec::new();
        let mut system_instruction = None;

        for msg in messages {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");

            match role {
                "system" => {
                    system_instruction = Some(serde_json::json!({
                        "parts": [{ "text": content }]
                    }));
                }
                _ => {
                    let gemini_role = match role {
                        "assistant" => "model",
                        _ => "user",
                    };
                    contents.push(serde_json::json!({
                        "role": gemini_role,
                        "parts": [{ "text": content }]
                    }));
                }
            }
        }

        let mut payload = serde_json::json!({ "contents": contents });

        if let Some(si) = system_instruction {
            payload.as_object_mut().unwrap().insert("systemInstruction".into(), si);
        }

        let mut generation_config = serde_json::json!({});
        if let Some(max_tokens) = openai_req.get("max_tokens") {
            generation_config.as_object_mut().unwrap().insert("maxOutputTokens".into(), max_tokens.clone());
        }
        if let Some(temp) = openai_req.get("temperature") {
            generation_config.as_object_mut().unwrap().insert("temperature".into(), temp.clone());
        }
        payload.as_object_mut().unwrap().insert("generationConfig".into(), generation_config);

        Ok(payload)
    }

    // ── OpenAI → Ollama ────────────────────────────────────────────────────

    pub fn to_ollama(openai_req: &Value) -> Result<Value, LlmError> {
        // Ollama's /api/chat accepts a similar format but uses "model" and "messages"
        let mut payload = serde_json::json!({
            "model": openai_req.get("model").unwrap_or(&Value::String("llama3".into())),
            "messages": openai_req.get("messages").unwrap_or(&Value::Array(vec![])),
        });
        if let Some(stream) = openai_req.get("stream") {
            payload.as_object_mut().unwrap().insert("stream".into(), stream.clone());
        }
        if let Some(temp) = openai_req.get("temperature") {
            payload.as_object_mut().unwrap().insert("options".into(), serde_json::json!({
                "temperature": temp,
            }));
        }
        Ok(payload)
    }

    // ── Response adapters (Provider → OpenAI format) ───────────────────────

    /// Convert Anthropic Messages API response to OpenAI chat completion format.
    ///
    /// Captures:
    /// - Primary text content
    /// - Reasoning ("thinking") blocks → surfaced as OpenAI `reasoning_details` array
    /// - Cache usage: `cache_creation_input_tokens` and `cache_read_input_tokens` → OpenAI cache fields
    /// - Tool use: collected into OpenAI `tool_calls`
    pub fn anthropic_response_to_openai(resp: &Value, model: &str) -> Value {
        let empty: Vec<Value> = vec![];
        let blocks = resp.get("content")
            .and_then(|c| c.as_array())
            .unwrap_or(&empty);

        // Extract text content + reasoning blocks separately
        let mut text_content = String::new();
        let mut reasoning_details: Vec<Value> = Vec::new();
        let mut tool_calls: Vec<Value> = Vec::new();
        let mut tool_idx = 0u64;

        for block in blocks {
            match block.get("type").and_then(|t| t.as_str()) {
                Some("text") => {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        text_content.push_str(t);
                    }
                }
                Some("thinking") => {
                    // Claude 3.7+ extended thinking
                    if let Some(t) = block.get("thinking").and_then(|v| v.as_str()) {
                        reasoning_details.push(serde_json::json!({
                            "type": "reasoning.text",
                            "text": t,
                        }));
                    }
                }
                Some("tool_use") => {
                    tool_calls.push(serde_json::json!({
                        "id": block.get("id").cloned().unwrap_or(Value::String(format!("call_{tool_idx}"))),
                        "type": "function",
                        "function": {
                            "name": block.get("name").cloned().unwrap_or(Value::String("".into())),
                            "arguments": serde_json::to_string(&block.get("input").cloned().unwrap_or(Value::Null)).unwrap_or_default(),
                        }
                    }));
                    tool_idx += 1;
                }
                _ => {}
            }
        }

        let stop_reason = resp.get("stop_reason")
            .and_then(|s| s.as_str())
            .unwrap_or("stop");

        let finish_reason = match stop_reason {
            "end_turn" | "stop_sequence" => "stop",
            "max_tokens" => "length",
            "tool_use" => "tool_calls",
            _ => "stop",
        };

        let usage = resp.get("usage").cloned().unwrap_or(serde_json::json!({}));
        let prompt_tokens = usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let completion_tokens = usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        // Anthropic prompt caching: separate counters for cache creation vs reads
        let cache_creation = usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        let cache_read = usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

        // Build the message object
        let mut message = serde_json::json!({
            "role": "assistant",
            "content": text_content,
        });
        if !tool_calls.is_empty() {
            message.as_object_mut().unwrap().insert("tool_calls".into(), Value::Array(tool_calls));
        }
        if !reasoning_details.is_empty() {
            message.as_object_mut().unwrap().insert("reasoning_details".into(), Value::Array(reasoning_details));
        }

        // Build usage with OpenAI-style cache + reasoning fields
        let mut usage_out = serde_json::json!({
            "prompt_tokens": prompt_tokens + cache_creation + cache_read,
            "completion_tokens": completion_tokens,
            "total_tokens": prompt_tokens + cache_creation + cache_read + completion_tokens,
        });
        if cache_creation > 0 || cache_read > 0 {
            // Match OpenAI `prompt_tokens_details.cached_tokens` convention
            usage_out.as_object_mut().unwrap().insert(
                "prompt_tokens_details".into(),
                serde_json::json!({
                    "cached_tokens": cache_read,
                    "cache_creation_tokens": cache_creation,
                }),
            );
        }

        serde_json::json!({
            "id": resp.get("id").unwrap_or(&Value::String("chatcmpl-sentinel".into())),
            "object": "chat.completion",
            "model": model,
            "choices": [{
                "index": 0,
                "message": message,
                "finish_reason": finish_reason,
            }],
            "usage": usage_out,
        })
    }

    /// Convert Gemini API response to OpenAI chat completion format.
    pub fn gemini_response_to_openai(resp: &Value, model: &str) -> Value {
        let content = resp.get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|candidate| candidate.get("content"))
            .and_then(|content| content.get("parts"))
            .and_then(|parts| parts.as_array())
            .and_then(|parts| parts.first())
            .and_then(|part| part.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");

        let finish_reason = resp.get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("finishReason"))
            .and_then(|r| r.as_str())
            .map(|r| match r { "STOP" => "stop", "MAX_TOKENS" => "length", _ => "stop" })
            .unwrap_or("stop");

        let usage_meta = resp.get("usageMetadata").cloned().unwrap_or(serde_json::json!({}));
        let prompt_tokens = usage_meta.get("promptTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);
        let completion_tokens = usage_meta.get("candidatesTokenCount").and_then(|v| v.as_u64()).unwrap_or(0);

        serde_json::json!({
            "id": "chatcmpl-sentinel",
            "object": "chat.completion",
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content
                },
                "finish_reason": finish_reason
            }],
            "usage": {
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": prompt_tokens + completion_tokens
            }
        })
    }

    /// Convert Ollama /api/chat response to OpenAI chat completion format.
    pub fn ollama_response_to_openai(resp: &Value) -> Value {
        let content = resp.get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("");

        let model = resp.get("model")
            .and_then(|m| m.as_str())
            .unwrap_or("ollama");

        let prompt_tokens = resp.get("prompt_eval_count").and_then(|v| v.as_u64()).unwrap_or(0);
        let completion_tokens = resp.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0);

        serde_json::json!({
            "id": "chatcmpl-sentinel",
            "object": "chat.completion",
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content
                },
                "finish_reason": if resp.get("done").and_then(|d| d.as_bool()).unwrap_or(true) { "stop" } else { "length" }
            }],
            "usage": {
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": prompt_tokens + completion_tokens
            }
        })
    }
}
