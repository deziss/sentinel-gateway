use regex::Regex;
use serde_json::Value;

/// Apply path rewriting based on route configuration.
///
/// 1. If `strip_prefix` is true, removes the `route_pattern` prefix from the path.
/// 2. Then applies regex-based rewrite rules from `rewrite_rules` JSON object.
///
/// `rewrite_rules` format: `{ "/pattern/(.*)": "/replacement/$1" }`
pub fn rewrite_path(
    original_path: &str,
    route_pattern: &str,
    strip_prefix: bool,
    rewrite_rules: &Value,
) -> String {
    let mut path = original_path.to_string();

    // Strip prefix
    if strip_prefix {
        if let Some(rest) = path.strip_prefix(route_pattern) {
            path = if rest.is_empty() || rest == "/" {
                "/".to_string()
            } else if rest.starts_with('/') {
                rest.to_string()
            } else {
                format!("/{rest}")
            };
        }
    }

    // Apply regex rewrite rules
    if let Some(rules) = rewrite_rules.as_object() {
        for (pattern, replacement) in rules {
            if let Some(rep) = replacement.as_str() {
                if let Ok(re) = Regex::new(pattern) {
                    let rewritten = re.replace(&path, rep).to_string();
                    if rewritten != path {
                        path = rewritten;
                        break; // Apply first matching rule only
                    }
                }
            }
        }
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_strip_prefix() {
        assert_eq!(rewrite_path("/api/v1/users", "/api/v1", true, &json!({})), "/users");
        assert_eq!(rewrite_path("/api/v1", "/api/v1", true, &json!({})), "/");
        assert_eq!(rewrite_path("/api/v1/", "/api/v1", true, &json!({})), "/");
    }

    #[test]
    fn test_no_strip() {
        assert_eq!(rewrite_path("/api/v1/users", "/api/v1", false, &json!({})), "/api/v1/users");
    }

    #[test]
    fn test_regex_rewrite() {
        let rules = json!({ "/old/(.*)": "/new/$1" });
        assert_eq!(rewrite_path("/old/users/123", "", false, &rules), "/new/users/123");
    }

    #[test]
    fn test_strip_then_rewrite() {
        let rules = json!({ "/users/(.*)": "/v2/users/$1" });
        assert_eq!(rewrite_path("/api/users/123", "/api", true, &rules), "/v2/users/123");
    }
}
