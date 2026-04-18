use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    // Auth events
    UserLogin,
    UserLogout,
    UserLoginFailed,
    PasswordChanged,
    // Resource events
    BackendCreated,
    BackendUpdated,
    BackendDeleted,
    RouteCreated,
    RouteDeleted,
    ApiKeyCreated,
    ApiKeyRevoked,
    UserInvited,
    UserDeactivated,
    // Proxy events
    ProxyRequest,
    ProxyError,
    // Policy events
    RateLimitExceeded,
    BudgetExceeded,
    IpBlocked,
    // Admin events
    TenantCreated,
    LicenseActivated,
    SettingsChanged,
    WebhookCreated,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub event_type: EventType,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl AuditEvent {
    pub fn new(
        tenant_id: Uuid,
        event_type: EventType,
        resource_type: impl Into<String>,
    ) -> Self {
        Self {
            tenant_id,
            user_id: None,
            event_type,
            resource_type: resource_type.into(),
            resource_id: None,
            details: serde_json::Value::Object(Default::default()),
            ip_address: None,
            user_agent: None,
            timestamp: Utc::now(),
        }
    }

    pub fn with_user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn with_resource_id(mut self, id: impl Into<String>) -> Self {
        self.resource_id = Some(id.into());
        self
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }
}
