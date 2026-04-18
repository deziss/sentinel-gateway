//! MCP session management.
//!
//! Each MCP client connection creates a session that tracks:
//! - Negotiated capabilities
//! - Client info
//! - Active subscriptions
//! - Session lifetime

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::protocol::{ClientCapabilities, ServerCapabilities, Implementation};

#[derive(Debug, Clone)]
pub struct McpSession {
    pub id: String,
    pub client_info: Implementation,
    pub client_capabilities: ClientCapabilities,
    pub server_capabilities: ServerCapabilities,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    /// Tenant ID for multi-tenant isolation
    pub tenant_id: Option<Uuid>,
    /// User ID for audit logging
    pub user_id: Option<Uuid>,
}

/// Thread-safe session store with automatic expiry.
#[derive(Clone)]
pub struct SessionStore {
    sessions: Arc<DashMap<String, McpSession>>,
    ttl_secs: u64,
}

impl SessionStore {
    pub fn new(ttl_secs: u64) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            ttl_secs,
        }
    }

    pub fn create(
        &self,
        client_info: Implementation,
        client_capabilities: ClientCapabilities,
        server_capabilities: ServerCapabilities,
        tenant_id: Option<Uuid>,
        user_id: Option<Uuid>,
    ) -> McpSession {
        let session = McpSession {
            id: Uuid::new_v4().to_string(),
            client_info,
            client_capabilities,
            server_capabilities,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            tenant_id,
            user_id,
        };
        self.sessions.insert(session.id.clone(), session.clone());
        session
    }

    pub fn get(&self, session_id: &str) -> Option<McpSession> {
        self.sessions.get(session_id).map(|s| {
            let mut session = s.clone();
            session.last_activity = Utc::now();
            session
        })
    }

    pub fn touch(&self, session_id: &str) {
        if let Some(mut s) = self.sessions.get_mut(session_id) {
            s.last_activity = Utc::now();
        }
    }

    pub fn remove(&self, session_id: &str) {
        self.sessions.remove(session_id);
    }

    /// Remove sessions that have been idle longer than TTL.
    pub fn cleanup_expired(&self) {
        let cutoff = Utc::now() - chrono::Duration::seconds(self.ttl_secs as i64);
        self.sessions.retain(|_, s| s.last_activity > cutoff);
    }

    pub fn count(&self) -> usize {
        self.sessions.len()
    }
}
