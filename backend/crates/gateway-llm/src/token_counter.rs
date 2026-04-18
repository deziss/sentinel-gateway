use tiktoken_rs::cl100k_base;

/// Precision token counter using tiktoken-rs.
pub struct TokenCounter;

impl TokenCounter {
    /// Accurate token count for a string using cl100k_base encoding.
    pub fn count(text: &str) -> u32 {
        let bpe = cl100k_base().unwrap();
        bpe.encode_with_special_tokens(text).len() as u32
    }

    /// Count tokens across a list of messages (OpenAI chat format).
    /// Follows the cl100k_base message overhead rules.
    pub fn count_messages(messages: &[serde_json::Value]) -> u32 {
        let mut total = 0u32;
        let bpe = cl100k_base().unwrap();

        for msg in messages {
            // cl100k_base overhead: 3 tokens per message + 1 for role if not role is 'name'
            total += 3;
            
            if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                total += bpe.encode_with_special_tokens(content).len() as u32;
            }
            if let Some(role) = msg.get("role").and_then(|r| r.as_str()) {
                total += bpe.encode_with_special_tokens(role).len() as u32;
            }
            if let Some(name) = msg.get("name").and_then(|n| n.as_str()) {
                total += bpe.encode_with_special_tokens(name).len() as u32;
                total += 1; // name overhead
            }
        }
        total += 3; // reply priming
        total
    }

    /// Count tokens in a completion response.
    pub fn count_completion(text: &str) -> u32 {
        Self::count(text)
    }
}
