use gateway_auth::TokenBlacklist;
use gateway_auth::ApiKeyCache;
use gateway_db::DbPool;
use gateway_db::repository::{
    ApiKeyRepository, AuditLogRepository, BackendRepository, GuardrailRuleRepository,
    PromptRepository, RouteRepository,
    SettingRepository, TenantRepository, UsageRecordRepository, UserRepository,
    WebhookEndpointRepository, WebhookFailureRepository,
    TeamRepository, VirtualKeyRepository, LlmLogRepository, TenantPricingRepository,
    SsoProviderRepository, SsoIdentityRepository, SsoAuthStateRepository,
    OrganizationRepository, LlmFeedbackRepository,
};
use gateway_audit::{AuditService, LlmLogService, DataLakeExporter};
use gateway_license::{ActivationService, DeploymentMode, FeatureFlags};
use gateway_telemetry::Metrics;
use gateway_auth::password::PasswordService;
use std::sync::Arc;

use crate::config::{AuthConfig, PlatformConfig, ServerConfig};

/// Shared application state injected into all Axum handlers.
///
/// Some fields (`password_service`, `platform_config`) are referenced only by
/// handlers we haven't written yet but are load-bearing for the init flow in
/// main.rs. `#[allow(dead_code)]` on the struct is simpler than propagating
/// per-field annotations that flip whenever a handler is added.
#[allow(dead_code)]
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
    pub llm_cache: Arc<gateway_llm::SemanticCache>,
    pub mcp_server: Arc<gateway_mcp::McpServer>,
    pub prompt_repo: Arc<PromptRepository>,
    pub guardrail_rule_repo: Arc<GuardrailRuleRepository>,
    pub team_repo: Arc<TeamRepository>,
    pub virtual_key_repo: Arc<VirtualKeyRepository>,
    pub llm_log_repo: Arc<LlmLogRepository>,
    pub tenant_pricing_repo: Arc<TenantPricingRepository>,
    pub llm_log_service: LlmLogService,
    pub sso_provider_repo: Arc<SsoProviderRepository>,
    pub sso_identity_repo: Arc<SsoIdentityRepository>,
    pub sso_auth_state_repo: Arc<SsoAuthStateRepository>,
    pub organization_repo: Arc<OrganizationRepository>,
    pub llm_feedback_repo: Arc<LlmFeedbackRepository>,
    pub data_lake: DataLakeExporter,
    /// Optional observability exporter (Langfuse / Helicone). `disabled()` if unconfigured.
    pub observability_exporter: crate::observability_export::ObservabilityExporter,
}
