//! MCP tool and resource registry with backend namespacing.
//!
//! Aggregates tools and resources from multiple upstream MCP servers into a
//! unified registry. Tools are namespaced by server name to avoid collisions:
//! e.g., `github__create_issue`, `slack__send_message`.

use dashmap::DashMap;
use std::sync::Arc;

use crate::protocol::{ToolDefinition, ResourceDefinition, PromptDefinition};

/// A registered upstream MCP server with its discovered primitives.
#[derive(Debug, Clone)]
pub struct McpBackend {
    pub id: String,
    pub name: String,
    pub url: String,
    pub tools: Vec<ToolDefinition>,
    pub resources: Vec<ResourceDefinition>,
    pub prompts: Vec<PromptDefinition>,
    pub is_healthy: bool,
}

/// Aggregates tools/resources from multiple MCP backends with namespacing.
#[derive(Clone)]
pub struct McpRegistry {
    backends: Arc<DashMap<String, McpBackend>>,
    /// Namespace separator (default: "__")
    separator: String,
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            backends: Arc::new(DashMap::new()),
            separator: "__".to_string(),
        }
    }

    /// Register or update an MCP backend with its discovered primitives.
    pub fn register_backend(&self, backend: McpBackend) {
        self.backends.insert(backend.id.clone(), backend);
    }

    /// Remove a backend from the registry.
    pub fn remove_backend(&self, backend_id: &str) {
        self.backends.remove(backend_id);
    }

    /// Mark a backend as healthy/unhealthy.
    pub fn set_health(&self, backend_id: &str, healthy: bool) {
        if let Some(mut b) = self.backends.get_mut(backend_id) {
            b.is_healthy = healthy;
        }
    }

    /// Get all tools across all healthy backends, namespaced by server name.
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        let mut tools = Vec::new();
        for entry in self.backends.iter() {
            let backend = entry.value();
            if !backend.is_healthy {
                continue;
            }
            for tool in &backend.tools {
                tools.push(ToolDefinition {
                    name: format!("{}{}{}", backend.name, self.separator, tool.name),
                    title: tool.title.clone(),
                    description: tool.description.as_ref().map(|d| {
                        format!("[{}] {}", backend.name, d)
                    }),
                    input_schema: tool.input_schema.clone(),
                });
            }
        }
        tools
    }

    /// Get all resources across all healthy backends, with namespaced URIs.
    pub fn list_resources(&self) -> Vec<ResourceDefinition> {
        let mut resources = Vec::new();
        for entry in self.backends.iter() {
            let backend = entry.value();
            if !backend.is_healthy {
                continue;
            }
            for res in &backend.resources {
                resources.push(ResourceDefinition {
                    uri: format!("mcp://{}/{}", backend.name, res.uri),
                    name: format!("{}{}{}", backend.name, self.separator, res.name),
                    description: res.description.clone(),
                    mime_type: res.mime_type.clone(),
                });
            }
        }
        resources
    }

    /// Get all prompts across all healthy backends.
    pub fn list_prompts(&self) -> Vec<PromptDefinition> {
        let mut prompts = Vec::new();
        for entry in self.backends.iter() {
            let backend = entry.value();
            if !backend.is_healthy {
                continue;
            }
            for prompt in &backend.prompts {
                prompts.push(PromptDefinition {
                    name: format!("{}{}{}", backend.name, self.separator, prompt.name),
                    description: prompt.description.clone(),
                    arguments: prompt.arguments.clone(),
                });
            }
        }
        prompts
    }

    /// Resolve a namespaced tool name to (backend_id, original_tool_name).
    pub fn resolve_tool(&self, namespaced_name: &str) -> Option<(String, String)> {
        for entry in self.backends.iter() {
            let backend = entry.value();
            let prefix = format!("{}{}", backend.name, self.separator);
            if let Some(tool_name) = namespaced_name.strip_prefix(&prefix) {
                if backend.tools.iter().any(|t| t.name == tool_name) {
                    return Some((backend.id.clone(), tool_name.to_string()));
                }
            }
        }
        None
    }

    /// Resolve a namespaced resource URI to (backend_id, original_uri).
    pub fn resolve_resource(&self, namespaced_uri: &str) -> Option<(String, String)> {
        // URI format: mcp://{server_name}/{original_uri}
        let stripped = namespaced_uri.strip_prefix("mcp://")?;
        let slash_pos = stripped.find('/')?;
        let server_name = &stripped[..slash_pos];
        let original_uri = &stripped[slash_pos + 1..];

        for entry in self.backends.iter() {
            let backend = entry.value();
            if backend.name == server_name {
                return Some((backend.id.clone(), original_uri.to_string()));
            }
        }
        None
    }

    /// Resolve a namespaced prompt name to (backend_id, original_prompt_name).
    pub fn resolve_prompt(&self, namespaced_name: &str) -> Option<(String, String)> {
        for entry in self.backends.iter() {
            let backend = entry.value();
            let prefix = format!("{}{}", backend.name, self.separator);
            if let Some(prompt_name) = namespaced_name.strip_prefix(&prefix) {
                if backend.prompts.iter().any(|p| p.name == prompt_name) {
                    return Some((backend.id.clone(), prompt_name.to_string()));
                }
            }
        }
        None
    }

    /// Get a backend by ID.
    pub fn get_backend(&self, backend_id: &str) -> Option<McpBackend> {
        self.backends.get(backend_id).map(|b| b.clone())
    }

    /// Get all registered backends.
    pub fn list_backends(&self) -> Vec<McpBackend> {
        self.backends.iter().map(|e| e.value().clone()).collect()
    }

    pub fn backend_count(&self) -> usize {
        self.backends.len()
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}
