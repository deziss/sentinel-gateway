//! Privacy-mode helpers: redact message bodies in logs while preserving
//! metadata (role, model, token counts) needed for billing and audit.

use serde_json::Value;

/// Redact all message content strings in an OpenAI-style chat request body.
/// Keeps the `role`, `tokens`, and top-level fields; replaces `content` with
/// a placeholder showing only the original length.
pub fn redact_request(body: &Value) -> Value {
    let mut out = body.clone();
    if let Some(messages) = out.get_mut("messages").and_then(|m| m.as_array_mut()) {
        for msg in messages {
            if let Some(obj) = msg.as_object_mut() {
                if let Some(content) = obj.get("content") {
                    let placeholder = redact_content(content);
                    obj.insert("content".to_string(), placeholder);
                }
            }
        }
    }
    if let Some(obj) = out.as_object_mut() {
        // Completions-style `prompt` field
        if let Some(p) = obj.get("prompt") {
            obj.insert("prompt".to_string(), redact_content(p));
        }
        // Embeddings `input`
        if let Some(inp) = obj.get("input") {
            obj.insert("input".to_string(), redact_content(inp));
        }
    }
    out
}

/// Redact the response body (choices[].message.content / text).
pub fn redact_response(body: &Value) -> Value {
    let mut out = body.clone();
    if let Some(choices) = out.get_mut("choices").and_then(|c| c.as_array_mut()) {
        for choice in choices {
            if let Some(obj) = choice.as_object_mut() {
                if let Some(msg) = obj.get_mut("message").and_then(|m| m.as_object_mut()) {
                    if let Some(c) = msg.get("content") {
                        let placeholder = redact_content(c);
                        msg.insert("content".to_string(), placeholder);
                    }
                }
                if let Some(text) = obj.get("text") {
                    let placeholder = redact_content(text);
                    obj.insert("text".to_string(), placeholder);
                }
            }
        }
    }
    // Embeddings responses: keep the vector shape but blank the data
    if let Some(data) = out.get_mut("data").and_then(|d| d.as_array_mut()) {
        for item in data {
            if let Some(obj) = item.as_object_mut() {
                if obj.contains_key("embedding") {
                    obj.insert("embedding".to_string(), Value::String("[redacted]".into()));
                }
            }
        }
    }
    out
}

fn redact_content(v: &Value) -> Value {
    match v {
        Value::String(s) => Value::String(format!("[redacted:{}chars]", s.len())),
        Value::Array(items) => {
            // Multimodal content parts: [{type: text, text: "..."}, ...]
            let redacted: Vec<Value> = items
                .iter()
                .map(|part| {
                    if let Some(obj) = part.as_object() {
                        let mut new_obj = obj.clone();
                        if let Some(t) = new_obj.get("text") {
                            new_obj.insert("text".to_string(), redact_content(t));
                        }
                        if new_obj.contains_key("image_url") {
                            new_obj.insert("image_url".to_string(), Value::String("[redacted]".into()));
                        }
                        Value::Object(new_obj)
                    } else {
                        part.clone()
                    }
                })
                .collect();
            Value::Array(redacted)
        }
        _ => v.clone(),
    }
}

/// Per-tenant cache key: namespace so tenant A can't read tenant B's cache.
pub fn tenant_cache_key(tenant_id: uuid::Uuid, request_fingerprint: &str) -> String {
    format!("{tenant_id}:{request_fingerprint}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_chat_messages_content() {
        let body = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "user", "content": "What is my SSN 123-45-6789?"},
                {"role": "assistant", "content": "I can't help with that."},
            ]
        });
        let out = redact_request(&body);
        let messages = out.get("messages").unwrap().as_array().unwrap();
        assert!(messages[0].get("content").unwrap().as_str().unwrap().starts_with("[redacted:"));
        assert!(messages[1].get("content").unwrap().as_str().unwrap().starts_with("[redacted:"));
        // Role is preserved
        assert_eq!(messages[0].get("role").unwrap(), "user");
        assert_eq!(messages[1].get("role").unwrap(), "assistant");
    }

    #[test]
    fn redacts_completions_prompt() {
        let body = json!({"model": "gpt-4", "prompt": "Write a poem about cats"});
        let out = redact_request(&body);
        assert!(out.get("prompt").unwrap().as_str().unwrap().starts_with("[redacted:"));
    }

    #[test]
    fn redacts_embeddings_input() {
        let body = json!({"model": "text-embedding-3-small", "input": "very secret text"});
        let out = redact_request(&body);
        assert!(out.get("input").unwrap().as_str().unwrap().starts_with("[redacted:"));
    }

    #[test]
    fn redacts_multimodal_content_parts() {
        let body = json!({
            "messages": [{
                "role": "user",
                "content": [
                    {"type": "text", "text": "describe this"},
                    {"type": "image_url", "image_url": "https://example.com/cat.jpg"},
                ]
            }]
        });
        let out = redact_request(&body);
        let parts = out["messages"][0]["content"].as_array().unwrap();
        assert!(parts[0]["text"].as_str().unwrap().starts_with("[redacted:"));
        assert_eq!(parts[1]["image_url"], "[redacted]");
    }

    #[test]
    fn preserves_non_content_fields() {
        let body = json!({
            "model": "gpt-4",
            "temperature": 0.7,
            "max_tokens": 100,
            "messages": [{"role": "user", "content": "hi"}],
        });
        let out = redact_request(&body);
        assert_eq!(out.get("model"), Some(&json!("gpt-4")));
        assert_eq!(out.get("temperature"), Some(&json!(0.7)));
        assert_eq!(out.get("max_tokens"), Some(&json!(100)));
    }

    #[test]
    fn redacts_chat_response_content() {
        let resp = json!({
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "The answer is 42"},
                "finish_reason": "stop",
            }]
        });
        let out = redact_response(&resp);
        let content = out["choices"][0]["message"]["content"].as_str().unwrap();
        assert!(content.starts_with("[redacted:"));
        // Role still visible; finish_reason still visible
        assert_eq!(out["choices"][0]["message"]["role"], "assistant");
        assert_eq!(out["choices"][0]["finish_reason"], "stop");
    }

    #[test]
    fn redacts_completions_text_response() {
        let resp = json!({
            "choices": [{"index": 0, "text": "generated poem", "finish_reason": "stop"}]
        });
        let out = redact_response(&resp);
        assert!(out["choices"][0]["text"].as_str().unwrap().starts_with("[redacted:"));
    }

    #[test]
    fn redacts_embeddings_vector() {
        let resp = json!({
            "data": [{"index": 0, "embedding": [0.1, 0.2, 0.3]}],
            "model": "text-embedding-3-small",
        });
        let out = redact_response(&resp);
        assert_eq!(out["data"][0]["embedding"], "[redacted]");
        // But index and model are preserved
        assert_eq!(out["data"][0]["index"], 0);
        assert_eq!(out["model"], "text-embedding-3-small");
    }

    #[test]
    fn tenant_cache_key_namespaces_by_tenant() {
        let t1 = uuid::Uuid::nil();
        let t2 = uuid::Uuid::from_u128(1);
        let k1 = tenant_cache_key(t1, "fingerprint");
        let k2 = tenant_cache_key(t2, "fingerprint");
        assert_ne!(k1, k2, "same fingerprint under different tenants must not collide");
        assert!(k1.ends_with(":fingerprint"));
    }

    #[test]
    fn redact_preserves_length_metadata() {
        let body = json!({"messages": [{"role": "user", "content": "hello"}]});
        let out = redact_request(&body);
        let placeholder = out["messages"][0]["content"].as_str().unwrap();
        assert_eq!(placeholder, "[redacted:5chars]");
    }

    #[test]
    fn redact_empty_body_does_not_panic() {
        let body = json!({});
        let _ = redact_request(&body);
        let _ = redact_response(&body);
    }
}
