use serde_json::Value;

/// Calculate the maximum nesting depth of a GraphQL query.
///
/// Counts brace depth `{ ... { ... } }` to determine query complexity.
/// Returns 0 for empty/invalid queries.
pub fn query_depth(query: &str) -> u32 {
    let mut depth = 0u32;
    let mut max_depth = 0u32;
    let mut in_string = false;
    let mut escape_next = false;

    for ch in query.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => {
                depth += 1;
                if depth > max_depth {
                    max_depth = depth;
                }
            }
            '}' if !in_string => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    max_depth
}

/// Check if a GraphQL query is an introspection query.
///
/// Detects `__schema`, `__type`, and `__typename` at the root level.
pub fn is_introspection(query: &str) -> bool {
    let lower = query.to_lowercase();
    lower.contains("__schema") || lower.contains("__type")
}

/// Extract the GraphQL query string from a JSON request body.
///
/// Supports both `{ "query": "..." }` and `{ "query": "...", "variables": {...} }`.
pub fn extract_query(body: &Value) -> Option<&str> {
    body.get("query").and_then(|q| q.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_depth() {
        assert_eq!(query_depth("{ users { name } }"), 2);
    }

    #[test]
    fn test_nested_depth() {
        assert_eq!(query_depth("{ users { posts { comments { text } } } }"), 4);
    }

    #[test]
    fn test_empty() {
        assert_eq!(query_depth(""), 0);
    }

    #[test]
    fn test_string_braces_ignored() {
        assert_eq!(query_depth(r#"{ user(name: "{}") { id } }"#), 2);
    }

    #[test]
    fn test_introspection() {
        assert!(is_introspection("{ __schema { types { name } } }"));
        assert!(is_introspection("{ __type(name: \"User\") { fields { name } } }"));
        assert!(!is_introspection("{ users { name } }"));
    }
}
