use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// PII detection mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PiiMode {
    /// Log a warning but don't modify the request.
    Detect,
    /// Replace PII with [REDACTED] placeholders.
    Redact,
    /// Reject the request entirely if PII is found.
    Block,
}

/// A detected PII match.
#[derive(Debug, Clone, Serialize)]
pub struct PiiMatch {
    pub pattern_type: &'static str,
    pub matched_text: String,
    pub start: usize,
    pub end: usize,
}

struct PiiPattern {
    name: &'static str,
    regex: Regex,
}

static PII_PATTERNS: Lazy<Vec<PiiPattern>> = Lazy::new(|| {
    vec![
        PiiPattern {
            name: "email",
            regex: Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap(),
        },
        PiiPattern {
            name: "phone_us",
            regex: Regex::new(r"\b(?:\+1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b").unwrap(),
        },
        PiiPattern {
            name: "phone_intl",
            regex: Regex::new(r"\+\d{1,3}[-.\s]?\d{4,14}").unwrap(),
        },
        PiiPattern {
            name: "ssn",
            regex: Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap(),
        },
        PiiPattern {
            name: "credit_card",
            regex: Regex::new(r"\b(?:\d{4}[-\s]?){3}\d{4}\b").unwrap(),
        },
        PiiPattern {
            name: "ipv4",
            regex: Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap(),
        },
        PiiPattern {
            name: "aws_key",
            regex: Regex::new(r"(?:AKIA|ABIA|ACCA)[0-9A-Z]{16}").unwrap(),
        },
        PiiPattern {
            name: "api_key_generic",
            regex: Regex::new(r#"(?i)(?:api[_-]?key|secret[_-]?key|access[_-]?token)\s*[:=]\s*['"]?[\w-]{20,}"#).unwrap(),
        },
    ]
});

/// Scan text for PII patterns. Returns all matches found.
pub fn detect(text: &str) -> Vec<PiiMatch> {
    let mut matches = Vec::new();
    for pattern in PII_PATTERNS.iter() {
        for m in pattern.regex.find_iter(text) {
            matches.push(PiiMatch {
                pattern_type: pattern.name,
                matched_text: m.as_str().to_string(),
                start: m.start(),
                end: m.end(),
            });
        }
    }
    matches
}

/// Redact PII from text, replacing matches with `[REDACTED:{type}]`.
pub fn redact(text: &str) -> String {
    let mut result = text.to_string();
    // Process in reverse order to preserve indices
    let mut all_matches: Vec<(usize, usize, &str)> = Vec::new();
    for pattern in PII_PATTERNS.iter() {
        for m in pattern.regex.find_iter(text) {
            all_matches.push((m.start(), m.end(), pattern.name));
        }
    }
    // Sort by start position descending
    all_matches.sort_by(|a, b| b.0.cmp(&a.0));
    for (start, end, name) in all_matches {
        result.replace_range(start..end, &format!("[REDACTED:{name}]"));
    }
    result
}

/// Scan messages array for PII. Returns matches across all message contents.
pub fn scan_messages(messages: &[serde_json::Value]) -> Vec<PiiMatch> {
    let mut all_matches = Vec::new();
    for msg in messages {
        if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
            all_matches.extend(detect(content));
        }
    }
    all_matches
}

/// Redact PII from all messages in a chat completion request.
/// Returns a new Value with redacted content.
pub fn redact_messages(request: &serde_json::Value) -> serde_json::Value {
    let mut req = request.clone();
    if let Some(messages) = req.get_mut("messages").and_then(|m| m.as_array_mut()) {
        for msg in messages.iter_mut() {
            if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                let redacted = redact(content);
                if let Some(obj) = msg.as_object_mut() {
                    obj.insert("content".to_string(), serde_json::Value::String(redacted));
                }
            }
        }
    }
    req
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_email() {
        let matches = detect("Contact me at john@example.com for details");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pattern_type, "email");
        assert_eq!(matches[0].matched_text, "john@example.com");
    }

    #[test]
    fn test_detect_ssn() {
        let matches = detect("SSN: 123-45-6789");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pattern_type, "ssn");
    }

    #[test]
    fn test_detect_credit_card() {
        let matches = detect("Card: 4111-1111-1111-1111");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].pattern_type, "credit_card");
    }

    #[test]
    fn test_redact() {
        let result = redact("Email john@example.com and SSN 123-45-6789");
        assert!(result.contains("[REDACTED:email]"));
        assert!(result.contains("[REDACTED:ssn]"));
        assert!(!result.contains("john@example.com"));
    }

    #[test]
    fn test_no_pii() {
        let matches = detect("Hello, how are you today?");
        assert!(matches.is_empty());
    }
}
