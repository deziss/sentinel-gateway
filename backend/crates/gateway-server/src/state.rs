use gateway_auth::TokenBlacklist;
use gateway_auth::ApiKeyCache;
use gateway_db::DbPool;
use gateway_db::repository::{
    ApiKeyRepository, AuditLogRepository, BackendRepository, GuardrailRuleRepository,
    PromptRepository, RouteRepository,
    SettingRepository, TenantRepository, UsageRecordRepository, UserRepository,
    WebhookEndpointRepository, WebhookFailureRepository,
};
use gateway_audit::AuditService;
use gateway_license::{ActivationService, DeploymentMode, FeatureFlags};
use gateway_telemetry::Metrics;
use gateway_auth::password::PasswordService;
use std::sync::Arc;

use crate::config::{AuthConfig, PlatformConfig, ServerConfig};

/// Shared application state injected into all Axum handlers.
pub struct AppState {
    pub db: DbPool,
    pub jwt: Arc<gateway_auth::JwtService>,
    pub token_blacklist: Arc<TokenBlacklist>,
    pub api_key_cache: Arc<ApiKeyCache>,
    pub features: Arc<FeatureFlags>,
    pub policy_engine: Arc<gateway_policy::PolicyEngine>,
    pub gateway_engine: Arc<gateway_core::GatewayEngine>,
    pub password_service: Arc<PasswordService>,
    pub health_checker: Arc<gateway_core::health::HealthChecker>,
    pub metrics: Arc<Metrics>,
    pub auth_config: AuthConfig,
    pub deployment_mode: DeploymentMode,
    pub server_config: ServerConfig,
    pub platform_config: PlatformConfig,

    // Services
    pub tenant_service: Arc<gateway_tenant::service::TenantService>,
    pub audit_service: Arc<AuditService>,
    pub tenant_repo: Arc<TenantRepository>,
    pub user_repo: Arc<UserRepository>,
    pub api_key_repo: Arc<ApiKeyRepository>,
    pub backend_repo: Arc<BackendRepository>,
    pub route_repo: Arc<RouteRepository>,
    pub audit_log_repo: Arc<AuditLogRepository>,
    pub setting_repo: Arc<SettingRepository>,
    pub usage_record_repo: Arc<UsageRecordRepository>,
    pub activation_service: Arc<ActivationService>,
    pub webhook_repo: Arc<WebhookEndpointRepository>,
    pub webhook_failure_repo: Arc<WebhookFailureRepository>,
    pub llm_router: Arc<gateway_llm::LlmRouter>,
    pub mcp_server: Arc<gateway_mcp::McpServer>,
    pub prompt_repo: Arc<PromptRepository>,
    pub guardrail_rule_repo: Arc<GuardrailRuleRepository>,
    /// Optional observability exporter (Langfuse / Helicone). `disabled()` if unconfigured.
    pub observability_exporter: crate::observability_export::ObservabilityExporter,
}
