use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Plan {
    Community,
    Professional,
    Enterprise,
}

impl Plan {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "professional" => Plan::Professional,
            "enterprise" => Plan::Enterprise,
            _ => Plan::Community,
        }
    }
}

/// How this instance is deployed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentMode {
    /// Fully offline, no license required, auto-provisioned single tenant.
    /// Uses Community plan features — generous, free forever.
    Local,
    /// Connected to platform, license-validated.
    /// Plan determined by license (Community / Professional / Enterprise).
    Platform,
}

/// Feature flags derived from the license plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub plan: Plan,
    // ── Quotas ──────────────────────────────────────────
    pub max_backends: u32,
    pub max_users: u32,
    pub max_api_keys: u32,
    pub max_requests_per_minute: u32,
    pub max_monthly_budget: f64,
    // ── Protocol support ────────────────────────────────
    pub graphql_enabled: bool,
    pub grpc_enabled: bool,
    // ── Enterprise features ─────────────────────────────
    pub sso_enabled: bool,
    pub multi_tenant: bool,
    pub custom_branding_enabled: bool,
    // ── Core features ───────────────────────────────────
    pub semantic_cache_enabled: bool,
    pub pii_detection_enabled: bool,
    pub prompt_versioning_enabled: bool,
    pub webhook_enabled: bool,
    pub ip_filtering_enabled: bool,
    pub budget_enforcement_enabled: bool,
    // ── LLM features ──────────────────────────────────────
    pub model_federation_enabled: bool,
    pub multi_provider_fallback: bool,
    pub cost_analytics_enabled: bool,
    pub custom_model_pricing: bool,
    pub embedding_support: bool,
    pub streaming_support: bool,
    // ── Protocol features ───────────────────────────────
    pub websocket_enabled: bool,
    pub http3_enabled: bool,
    // ── Retention ───────────────────────────────────────
    pub audit_log_retention_days: u32,
}

impl FeatureFlags {
    pub fn for_plan(plan: Plan) -> Self {
        match plan {
            // Community: generous free tier — all core features, unlimited quotas.
            // Only enterprise-class features are gated.
            Plan::Community => Self {
                plan: Plan::Community,
                max_backends: u32::MAX,
                max_users: u32::MAX,
                max_api_keys: u32::MAX,
                max_requests_per_minute: u32::MAX,
                max_monthly_budget: f64::MAX,
                sso_enabled: false,
                graphql_enabled: true,
                grpc_enabled: false,
                multi_tenant: false,
                custom_branding_enabled: false,
                semantic_cache_enabled: true,
                pii_detection_enabled: true,
                prompt_versioning_enabled: true,
                webhook_enabled: true,
                ip_filtering_enabled: true,
                budget_enforcement_enabled: true,
                model_federation_enabled: false,
                multi_provider_fallback: false,
                cost_analytics_enabled: false,
                custom_model_pricing: false,
                embedding_support: true,
                streaming_support: true,
                websocket_enabled: true,
                http3_enabled: false,
                audit_log_retention_days: 30,
            },
            Plan::Professional => Self {
                plan: Plan::Professional,
                max_backends: 20,
                max_users: 50,
                max_api_keys: 200,
                max_requests_per_minute: 1000,
                max_monthly_budget: 500.0,
                sso_enabled: true,
                graphql_enabled: true,
                grpc_enabled: false,
                multi_tenant: true,
                custom_branding_enabled: false,
                semantic_cache_enabled: true,
                pii_detection_enabled: true,
                prompt_versioning_enabled: true,
                webhook_enabled: true,
                ip_filtering_enabled: true,
                budget_enforcement_enabled: true,
                model_federation_enabled: false,
                multi_provider_fallback: true,
                cost_analytics_enabled: true,
                custom_model_pricing: false,
                embedding_support: true,
                streaming_support: true,
                websocket_enabled: true,
                http3_enabled: false,
                audit_log_retention_days: 90,
            },
            Plan::Enterprise => Self {
                plan: Plan::Enterprise,
                max_backends: u32::MAX,
                max_users: u32::MAX,
                max_api_keys: u32::MAX,
                max_requests_per_minute: u32::MAX,
                max_monthly_budget: f64::MAX,
                sso_enabled: true,
                graphql_enabled: true,
                grpc_enabled: true,
                multi_tenant: true,
                custom_branding_enabled: true,
                semantic_cache_enabled: true,
                pii_detection_enabled: true,
                prompt_versioning_enabled: true,
                webhook_enabled: true,
                ip_filtering_enabled: true,
                budget_enforcement_enabled: true,
                model_federation_enabled: true,
                multi_provider_fallback: true,
                cost_analytics_enabled: true,
                custom_model_pricing: true,
                embedding_support: true,
                streaming_support: true,
                websocket_enabled: true,
                http3_enabled: true,
                audit_log_retention_days: 365,
            },
        }
    }

    pub fn requires_graphql(&self) -> bool {
        self.graphql_enabled
    }

    pub fn requires_grpc(&self) -> bool {
        self.grpc_enabled
    }
}
