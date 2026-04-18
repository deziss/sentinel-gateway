use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "route_protocol", rename_all = "snake_case")]
pub enum RouteProtocol {
    Rest,
    Graphql,
    Grpc,
    Generic,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Route {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub protocol: RouteProtocol,
    pub path_pattern: String,
    pub backend_id: Uuid,
    pub strip_prefix: bool,
    pub rewrite_rules: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoute {
    pub tenant_id: Uuid,
    pub name: String,
    pub protocol: RouteProtocol,
    pub path_pattern: String,
    pub backend_id: Uuid,
    pub strip_prefix: bool,
    pub rewrite_rules: serde_json::Value,
}
