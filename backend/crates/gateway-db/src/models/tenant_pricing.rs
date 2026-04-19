use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Per-tenant pricing override for a specific model.
/// Overrides NULL fall back to the built-in `cost.rs` pricing table.
/// `markup_multiplier` is always applied (default 1.0 = no markup).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TenantPricing {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub model: String,
    pub input_per_1m: Option<f64>,
    pub output_per_1m: Option<f64>,
    pub markup_multiplier: f64,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertTenantPricing {
    pub tenant_id: Uuid,
    pub model: String,
    pub input_per_1m: Option<f64>,
    pub output_per_1m: Option<f64>,
    pub markup_multiplier: f64,
}
