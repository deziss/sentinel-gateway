use serde::{Deserialize, Serialize};

/// Commercial plan tier. Determines which features are enabled.
///
/// Three tiers (no "Dev" free-cloud tier — self-hosted OSS replaces it):
///   - **Community** — Open Source self-hosted. Core gateway routing only.
///   - **Professional** — Paid. Observability, prompt mgmt, guardrails, RBAC.
///   - **Enterprise** — Custom. Everything + compliance + org management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Plan {
    Community,
    Professional,
    Enterprise,
}

impl Plan {
    /// Parse a plan tier from a string, defaulting to `Community` on any unrecognized input.
    ///
    /// Infallible by design: unknown plan names (including empty/whitespace) degrade to the
    /// safe free tier rather than erroring. This is NOT `std::str::FromStr`; that trait would
    /// require a `Result` return, and callers across the gateway rely on graceful fallback.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "professional" | "pro" => Plan::Professional,
            "enterprise" => Plan::Enterprise,
            _ => Plan::Community,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Plan::Community => "community",
            Plan::Professional => "professional",
            Plan::Enterprise => "enterprise",
        }
    }

    /// Plan hierarchy: Enterprise >= Professional >= Community.
    pub fn rank(&self) -> u8 {
        match self {
            Plan::Community => 0,
            Plan::Professional => 1,
            Plan::Enterprise => 2,
        }
    }

    /// True if this plan includes all features of `required` or higher.
    pub fn meets(&self, required: Plan) -> bool {
        self.rank() >= required.rank()
    }
}

/// How this instance is deployed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentMode {
    /// Community edition — fully offline, no license required, core features only.
    Local,
    /// PaaS mode — developer/superadmin, all features unlocked via developer secret.
    PaaS,
    Platform,
}

/// Feature flags and quotas for the current license.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub plan: Plan,
    
    // Quotas
    pub max_requests_per_month: u64,
    pub max_backends: u32,
    pub max_users: u32,
    pub max_api_keys: u32,
    pub max_requests_per_minute: u32,
    pub max_monthly_budget: f64,
    pub retention_days: u32,

    // Core Gateway
    pub universal_api: bool,
    pub automatic_fallbacks: bool,
    pub loadbalancing: bool,
    pub conditional_routing: bool,
    pub automatic_retries: bool,
    pub request_timeouts: bool,
    pub multi_provider_fallback: bool,

    // Management
    pub config_management: bool,
    pub llm_key_management: bool,
    pub admin_apis_enabled: bool,
    pub dashboard_enabled: bool,

    // Caching
    pub simple_cache_enabled: bool,
    pub semantic_cache_enabled: bool,
    pub cache_max_ttl_secs: u64,

    // Observability
    pub logs_enabled: bool,
    pub traces_enabled: bool,
    pub feedback_enabled: bool,
    pub custom_metadata_enabled: bool,
    pub filters_enabled: bool,
    pub alerts_enabled: bool,
    pub finops_dashboard_enabled: bool,
    pub audit_logs_enabled: bool,
    pub datalake_export_enabled: bool,

    // LLM Advanced
    pub prompt_templates_enabled: bool,
    pub max_prompt_templates: u32,
    pub playground_enabled: bool,
    pub prompt_api_deployment: bool,
    pub prompt_versioning_enabled: bool,
    pub prompt_variables_enabled: bool,
    pub prompt_partials_enabled: bool,
    pub prompt_side_by_side_enabled: bool,
    pub prompt_access_control: bool,
    pub deterministic_guardrails: bool,
    pub partner_guardrails: bool,
    pub pii_redaction_enabled: bool,
    pub unified_fine_tuning_batch: bool,
    pub private_llm_cloud: bool,
    pub autonomous_fine_tuning: bool,

    // Security & Enterprise
    pub rbac_enabled: bool,
    pub rbac_advanced: bool,
    pub team_management: bool,
    pub team_management_advanced: bool,
    pub scim_provisioning: bool,
    pub jwt_auth_enabled: bool,
    pub byok_enabled: bool,
    pub sso_enabled: bool,
    pub org_metadata_reporting: bool,
    pub org_llm_guardrails: bool,
    pub compliance_certs: bool,
    pub baa_signing: bool,
    pub vpc_managed_hosting: bool,
    pub private_tenancy: bool,
    pub configurable_retention: bool,
    pub org_management_enabled: bool,

    // Legacy / Other
    pub graphql_enabled: bool,
    pub grpc_enabled: bool,
    pub multi_tenant: bool,
    pub custom_branding_enabled: bool,
    pub webhook_enabled: bool,
    pub ip_filtering_enabled: bool,
    pub budget_enforcement_enabled: bool,
    pub model_federation_enabled: bool,
    pub cost_analytics_enabled: bool,
    pub custom_model_pricing: bool,
    pub embedding_support: bool,
    pub streaming_support: bool,
    pub websocket_enabled: bool,
    pub http3_enabled: bool,
    pub audit_log_retention_days: u32,
}

impl FeatureFlags {
    /// All features ON, all quotas unlimited. Used in PaaS/developer mode.
    pub fn all_unlocked() -> Self {
        Self::for_plan(Plan::Enterprise)
    }

    pub fn for_plan(plan: Plan) -> Self {
        match plan {
            // ── Community (Open Source self-hosted) ───────────────────
            // Core gateway routing only. Everything else off.
            // No quotas — users self-host and bring their own infra.
            Plan::Community => Self {
                plan: Plan::Community,
                max_requests_per_month: u64::MAX,
                max_backends: u32::MAX,
                max_users: u32::MAX,
                max_api_keys: u32::MAX,
                max_requests_per_minute: u32::MAX,
                max_monthly_budget: f64::MAX,

                universal_api: true,
                automatic_fallbacks: true,
                loadbalancing: true,
                conditional_routing: true,
                automatic_retries: true,
                request_timeouts: true,
                config_management: false,
                llm_key_management: false,
                dashboard_enabled: false,

                simple_cache_enabled: false,
                semantic_cache_enabled: false,
                cache_max_ttl_secs: 0,

                logs_enabled: false,
                traces_enabled: false,
                feedback_enabled: false,
                custom_metadata_enabled: false,
                filters_enabled: false,
                alerts_enabled: false,
                finops_dashboard_enabled: false,
                retention_days: 0,

                prompt_templates_enabled: false,
                max_prompt_templates: 0,
                playground_enabled: false,
                prompt_api_deployment: false,
                prompt_versioning_enabled: false,
                prompt_variables_enabled: false,
                prompt_partials_enabled: false,
                prompt_side_by_side_enabled: false,
                prompt_access_control: false,

                deterministic_guardrails: false,
                partner_guardrails: false,
                pii_redaction_enabled: false,

                unified_fine_tuning_batch: false,
                private_llm_cloud: false,
                autonomous_fine_tuning: false,

                rbac_enabled: false,
                rbac_advanced: false,
                team_management: false,
                team_management_advanced: false,
                audit_logs_enabled: false,
                admin_apis_enabled: false,
                scim_provisioning: false,
                jwt_auth_enabled: false,
                byok_enabled: false,
                org_metadata_reporting: false,
                org_llm_guardrails: false,
                sso_enabled: false,
                compliance_certs: false,
                baa_signing: false,
                vpc_managed_hosting: false,
                private_tenancy: false,
                configurable_retention: false,
                datalake_export_enabled: false,
                org_management_enabled: false,

                // Legacy
                graphql_enabled: false,
                grpc_enabled: false,
                multi_tenant: false,
                custom_branding_enabled: false,
                webhook_enabled: false,
                ip_filtering_enabled: false,
                budget_enforcement_enabled: false,
                model_federation_enabled: false,
                multi_provider_fallback: true, // Auto-fallback is core gateway
                cost_analytics_enabled: false,
                custom_model_pricing: false,
                embedding_support: true,
                streaming_support: true,
                websocket_enabled: false,
                http3_enabled: false,
                audit_log_retention_days: 0,
            },

            // ── Professional ($49/mo equivalent) ──────────────────────
            // 100K requests/month. All observability (except FinOps/exec dashboard),
            // caching (incl. semantic), prompt mgmt, guardrails (+PII), RBAC, teams.
            Plan::Professional => Self {
                plan: Plan::Professional,
                max_requests_per_month: 100_000,
                max_backends: u32::MAX,
                max_users: u32::MAX,
                max_api_keys: u32::MAX,
                max_requests_per_minute: u32::MAX,
                max_monthly_budget: f64::MAX,

                universal_api: true,
                automatic_fallbacks: true,
                loadbalancing: true,
                conditional_routing: true,
                automatic_retries: true,
                request_timeouts: true,
                config_management: true,
                llm_key_management: true,
                dashboard_enabled: true,

                simple_cache_enabled: true,
                semantic_cache_enabled: true,
                cache_max_ttl_secs: u64::MAX,

                logs_enabled: true,
                traces_enabled: true,
                feedback_enabled: true,
                custom_metadata_enabled: true,
                filters_enabled: true,
                alerts_enabled: true,
                finops_dashboard_enabled: false,
                retention_days: 30,

                prompt_templates_enabled: true,
                max_prompt_templates: u32::MAX,
                playground_enabled: true,
                prompt_api_deployment: true,
                prompt_versioning_enabled: true,
                prompt_variables_enabled: true,
                prompt_partials_enabled: true,
                prompt_side_by_side_enabled: true,
                prompt_access_control: true,

                deterministic_guardrails: true,
                partner_guardrails: true,
                pii_redaction_enabled: true,

                unified_fine_tuning_batch: false,
                private_llm_cloud: false,
                autonomous_fine_tuning: false,

                rbac_enabled: true,
                rbac_advanced: false,
                team_management: true,
                team_management_advanced: false,
                audit_logs_enabled: false,
                admin_apis_enabled: false,
                scim_provisioning: false,
                jwt_auth_enabled: false,
                byok_enabled: false,
                org_metadata_reporting: false,
                org_llm_guardrails: false,
                sso_enabled: false,
                compliance_certs: false,
                baa_signing: false,
                vpc_managed_hosting: false,
                private_tenancy: false,
                configurable_retention: false,
                datalake_export_enabled: false,
                org_management_enabled: false,

                // Legacy
                graphql_enabled: true,
                grpc_enabled: false,
                multi_tenant: true,
                custom_branding_enabled: false,
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
                audit_log_retention_days: 30,
            },

            // ── Enterprise (custom) ────────────────────────────────────
            // Everything ON, unlimited quotas.
            Plan::Enterprise => Self {
                plan: Plan::Enterprise,
                max_requests_per_month: u64::MAX,
                max_backends: u32::MAX,
                max_users: u32::MAX,
                max_api_keys: u32::MAX,
                max_requests_per_minute: u32::MAX,
                max_monthly_budget: f64::MAX,

                universal_api: true,
                automatic_fallbacks: true,
                loadbalancing: true,
                conditional_routing: true,
                automatic_retries: true,
                request_timeouts: true,
                config_management: true,
                llm_key_management: true,
                admin_apis_enabled: true,
                dashboard_enabled: true,

                simple_cache_enabled: true,
                semantic_cache_enabled: true,
                cache_max_ttl_secs: u64::MAX,

                logs_enabled: true,
                traces_enabled: true,
                feedback_enabled: true,
                custom_metadata_enabled: true,
                filters_enabled: true,
                alerts_enabled: true,
                finops_dashboard_enabled: true,
                retention_days: u32::MAX,

                prompt_templates_enabled: true,
                max_prompt_templates: u32::MAX,
                playground_enabled: true,
                prompt_api_deployment: true,
                prompt_versioning_enabled: true,
                prompt_variables_enabled: true,
                prompt_partials_enabled: true,
                prompt_side_by_side_enabled: true,
                prompt_access_control: true,

                deterministic_guardrails: true,
                partner_guardrails: true,
                pii_redaction_enabled: true,

                unified_fine_tuning_batch: true,
                private_llm_cloud: true,
                autonomous_fine_tuning: true,

                rbac_enabled: true,
                rbac_advanced: true,
                team_management: true,
                team_management_advanced: true,
                audit_logs_enabled: true,
                scim_provisioning: true,
                jwt_auth_enabled: true,
                byok_enabled: true,
                org_metadata_reporting: true,
                org_llm_guardrails: true,
                sso_enabled: true,
                compliance_certs: true,
                baa_signing: true,
                vpc_managed_hosting: true,
                private_tenancy: true,
                configurable_retention: true,
                datalake_export_enabled: true,
                org_management_enabled: true,

                // Legacy
                graphql_enabled: true,
                grpc_enabled: true,
                multi_tenant: true,
                custom_branding_enabled: true,
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
                audit_log_retention_days: u32::MAX,
            },
        }
    }

    pub fn requires_graphql(&self) -> bool { self.graphql_enabled }
    pub fn requires_grpc(&self) -> bool { self.grpc_enabled }
}

/// Named feature identifiers used by `require_feature()` guards and the
/// frontend feature matrix. Adding a variant requires a `check()` mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    // Observability
    Logs,
    Traces,
    Feedback,
    Alerts,
    FinopsDashboard,
    // Gateway
    LlmKeyManagement,
    SimpleCache,
    SemanticCache,
    // Prompts
    PromptTemplates,
    Playground,
    // Guardrails
    DeterministicGuardrails,
    PartnerGuardrails,
    PiiRedaction,
    // Security
    Rbac,
    TeamManagement,
    AuditLogs,
    AdminApis,
    Scim,
    JwtAuth,
    Byok,
    Sso,
    // Enterprise-exclusive
    OrgManagement,
    DatalakeExport,
    PrivateLlm,
    BaaSigning,
    VpcHosting,
    ConfigurableRetention,
    AutonomousFineTuning,
}

impl Feature {
    /// Check whether a given plan's flags enable this feature.
    pub fn check(&self, f: &FeatureFlags) -> bool {
        match self {
            Feature::Logs => f.logs_enabled,
            Feature::Traces => f.traces_enabled,
            Feature::Feedback => f.feedback_enabled,
            Feature::Alerts => f.alerts_enabled,
            Feature::FinopsDashboard => f.finops_dashboard_enabled,
            Feature::LlmKeyManagement => f.llm_key_management,
            Feature::SimpleCache => f.simple_cache_enabled,
            Feature::SemanticCache => f.semantic_cache_enabled,
            Feature::PromptTemplates => f.prompt_templates_enabled,
            Feature::Playground => f.playground_enabled,
            Feature::DeterministicGuardrails => f.deterministic_guardrails,
            Feature::PartnerGuardrails => f.partner_guardrails,
            Feature::PiiRedaction => f.pii_redaction_enabled,
            Feature::Rbac => f.rbac_enabled,
            Feature::TeamManagement => f.team_management,
            Feature::AuditLogs => f.audit_logs_enabled,
            Feature::AdminApis => f.admin_apis_enabled,
            Feature::Scim => f.scim_provisioning,
            Feature::JwtAuth => f.jwt_auth_enabled,
            Feature::Byok => f.byok_enabled,
            Feature::Sso => f.sso_enabled,
            Feature::OrgManagement => f.org_management_enabled,
            Feature::DatalakeExport => f.datalake_export_enabled,
            Feature::PrivateLlm => f.private_llm_cloud,
            Feature::BaaSigning => f.baa_signing,
            Feature::VpcHosting => f.vpc_managed_hosting,
            Feature::ConfigurableRetention => f.configurable_retention,
            Feature::AutonomousFineTuning => f.autonomous_fine_tuning,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Feature::Logs => "logs",
            Feature::Traces => "traces",
            Feature::Feedback => "feedback",
            Feature::Alerts => "alerts",
            Feature::FinopsDashboard => "finops_dashboard",
            Feature::LlmKeyManagement => "llm_key_management",
            Feature::SimpleCache => "simple_cache",
            Feature::SemanticCache => "semantic_cache",
            Feature::PromptTemplates => "prompt_templates",
            Feature::Playground => "playground",
            Feature::DeterministicGuardrails => "deterministic_guardrails",
            Feature::PartnerGuardrails => "partner_guardrails",
            Feature::PiiRedaction => "pii_redaction",
            Feature::Rbac => "rbac",
            Feature::TeamManagement => "team_management",
            Feature::AuditLogs => "audit_logs",
            Feature::AdminApis => "admin_apis",
            Feature::Scim => "scim",
            Feature::JwtAuth => "jwt_auth",
            Feature::Byok => "byok",
            Feature::Sso => "sso",
            Feature::OrgManagement => "org_management",
            Feature::DatalakeExport => "datalake_export",
            Feature::PrivateLlm => "private_llm",
            Feature::BaaSigning => "baa_signing",
            Feature::VpcHosting => "vpc_hosting",
            Feature::ConfigurableRetention => "configurable_retention",
            Feature::AutonomousFineTuning => "autonomous_fine_tuning",
        }
    }

    /// Lowest plan that grants this feature.
    pub fn min_plan(&self) -> Plan {
        match self {
            // All Pro+
            Feature::Logs | Feature::Traces | Feature::Feedback | Feature::Alerts
            | Feature::LlmKeyManagement | Feature::SimpleCache | Feature::SemanticCache
            | Feature::PromptTemplates | Feature::Playground
            | Feature::DeterministicGuardrails | Feature::PartnerGuardrails | Feature::PiiRedaction
            | Feature::Rbac | Feature::TeamManagement
            => Plan::Professional,

            // Enterprise-only
            Feature::FinopsDashboard | Feature::AuditLogs | Feature::AdminApis
            | Feature::Scim | Feature::JwtAuth | Feature::Byok | Feature::Sso
            | Feature::OrgManagement | Feature::DatalakeExport | Feature::PrivateLlm
            | Feature::BaaSigning | Feature::VpcHosting | Feature::ConfigurableRetention
            | Feature::AutonomousFineTuning
            => Plan::Enterprise,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_from_str_aliases() {
        assert_eq!(Plan::from_str("pro"), Plan::Professional);
        assert_eq!(Plan::from_str("Professional"), Plan::Professional);
        assert_eq!(Plan::from_str("ENTERPRISE"), Plan::Enterprise);
        assert_eq!(Plan::from_str("garbage"), Plan::Community);
    }

    #[test]
    fn plan_meets_is_transitive() {
        assert!(Plan::Enterprise.meets(Plan::Community));
        assert!(Plan::Enterprise.meets(Plan::Professional));
        assert!(Plan::Enterprise.meets(Plan::Enterprise));
        assert!(Plan::Professional.meets(Plan::Community));
        assert!(Plan::Professional.meets(Plan::Professional));
        assert!(!Plan::Professional.meets(Plan::Enterprise));
        assert!(!Plan::Community.meets(Plan::Professional));
        assert!(!Plan::Community.meets(Plan::Enterprise));
    }

    // ── Community (OSS) ──────────────────────────────────────────────

    #[test]
    fn community_has_core_gateway_only() {
        let f = FeatureFlags::for_plan(Plan::Community);
        assert!(f.universal_api);
        assert!(f.automatic_fallbacks);
        assert!(f.loadbalancing);
        assert!(f.conditional_routing);
        assert!(f.automatic_retries);
        assert!(f.request_timeouts);
    }

    #[test]
    fn community_has_no_observability() {
        let f = FeatureFlags::for_plan(Plan::Community);
        assert!(!f.logs_enabled);
        assert!(!f.traces_enabled);
        assert!(!f.feedback_enabled);
        assert!(!f.alerts_enabled);
        assert!(!f.finops_dashboard_enabled);
    }

    #[test]
    fn community_has_no_prompt_management() {
        let f = FeatureFlags::for_plan(Plan::Community);
        assert!(!f.prompt_templates_enabled);
        assert!(!f.playground_enabled);
        assert!(!f.prompt_versioning_enabled);
    }

    #[test]
    fn community_has_no_guardrails() {
        let f = FeatureFlags::for_plan(Plan::Community);
        assert!(!f.deterministic_guardrails);
        assert!(!f.partner_guardrails);
        assert!(!f.pii_redaction_enabled);
    }

    #[test]
    fn community_has_no_security_features() {
        let f = FeatureFlags::for_plan(Plan::Community);
        assert!(!f.rbac_enabled);
        assert!(!f.team_management);
        assert!(!f.sso_enabled);
        assert!(!f.audit_logs_enabled);
    }

    // ── Professional ─────────────────────────────────────────────────

    #[test]
    fn professional_has_observability_except_finops() {
        let f = FeatureFlags::for_plan(Plan::Professional);
        assert!(f.logs_enabled);
        assert!(f.traces_enabled);
        assert!(f.feedback_enabled);
        assert!(f.alerts_enabled);
        assert!(!f.finops_dashboard_enabled); // Enterprise-only
    }

    #[test]
    fn professional_has_full_caching() {
        let f = FeatureFlags::for_plan(Plan::Professional);
        assert!(f.simple_cache_enabled);
        assert!(f.semantic_cache_enabled);
        assert_eq!(f.cache_max_ttl_secs, u64::MAX);
    }

    #[test]
    fn professional_has_prompt_mgmt_with_unlimited_templates() {
        let f = FeatureFlags::for_plan(Plan::Professional);
        assert!(f.prompt_templates_enabled);
        assert_eq!(f.max_prompt_templates, u32::MAX);
        assert!(f.prompt_versioning_enabled);
        assert!(f.prompt_side_by_side_enabled);
    }

    #[test]
    fn professional_has_pii_redaction() {
        let f = FeatureFlags::for_plan(Plan::Professional);
        assert!(f.pii_redaction_enabled);
        assert!(f.deterministic_guardrails);
        assert!(f.partner_guardrails);
    }

    #[test]
    fn professional_has_rbac_and_teams() {
        let f = FeatureFlags::for_plan(Plan::Professional);
        assert!(f.rbac_enabled);
        assert!(!f.rbac_advanced);
        assert!(f.team_management);
        assert!(!f.team_management_advanced);
    }

    #[test]
    fn professional_lacks_enterprise_security() {
        let f = FeatureFlags::for_plan(Plan::Professional);
        assert!(!f.sso_enabled);
        assert!(!f.audit_logs_enabled);
        assert!(!f.scim_provisioning);
        assert!(!f.jwt_auth_enabled);
        assert!(!f.byok_enabled);
        assert!(!f.org_management_enabled);
        assert!(!f.datalake_export_enabled);
    }

    #[test]
    fn professional_monthly_quota_is_100k() {
        let f = FeatureFlags::for_plan(Plan::Professional);
        assert_eq!(f.max_requests_per_month, 100_000);
        assert_eq!(f.retention_days, 30);
    }

    // ── Enterprise ───────────────────────────────────────────────────

    #[test]
    fn enterprise_has_everything() {
        let f = FeatureFlags::for_plan(Plan::Enterprise);
        // Spot check — compliance, private hosting, advanced RBAC, etc.
        assert!(f.finops_dashboard_enabled);
        assert!(f.audit_logs_enabled);
        assert!(f.scim_provisioning);
        assert!(f.jwt_auth_enabled);
        assert!(f.byok_enabled);
        assert!(f.sso_enabled);
        assert!(f.org_management_enabled);
        assert!(f.datalake_export_enabled);
        assert!(f.rbac_advanced);
        assert!(f.team_management_advanced);
        assert!(f.compliance_certs);
        assert!(f.baa_signing);
        assert!(f.vpc_managed_hosting);
        assert!(f.private_tenancy);
        assert!(f.autonomous_fine_tuning);
        assert!(f.private_llm_cloud);
        assert!(f.unified_fine_tuning_batch);
    }

    #[test]
    fn enterprise_has_unlimited_quotas() {
        let f = FeatureFlags::for_plan(Plan::Enterprise);
        assert_eq!(f.max_requests_per_month, u64::MAX);
        assert_eq!(f.retention_days, u32::MAX);
    }

    // ── Feature::check + min_plan ────────────────────────────────────

    #[test]
    fn feature_check_matches_plan_flags() {
        let community = FeatureFlags::for_plan(Plan::Community);
        let pro = FeatureFlags::for_plan(Plan::Professional);
        let enterprise = FeatureFlags::for_plan(Plan::Enterprise);

        assert!(!Feature::Feedback.check(&community));
        assert!(Feature::Feedback.check(&pro));
        assert!(Feature::Feedback.check(&enterprise));

        assert!(!Feature::Sso.check(&community));
        assert!(!Feature::Sso.check(&pro));
        assert!(Feature::Sso.check(&enterprise));
    }

    #[test]
    fn feature_min_plan_is_consistent_with_for_plan() {
        // For every Feature, a plan meeting min_plan() must have the flag set.
        let all = [
            Feature::Logs, Feature::Traces, Feature::Feedback, Feature::Alerts,
            Feature::FinopsDashboard, Feature::LlmKeyManagement, Feature::SimpleCache,
            Feature::SemanticCache, Feature::PromptTemplates, Feature::Playground,
            Feature::DeterministicGuardrails, Feature::PartnerGuardrails, Feature::PiiRedaction,
            Feature::Rbac, Feature::TeamManagement, Feature::AuditLogs, Feature::AdminApis,
            Feature::Scim, Feature::JwtAuth, Feature::Byok, Feature::Sso,
            Feature::OrgManagement, Feature::DatalakeExport, Feature::PrivateLlm,
            Feature::BaaSigning, Feature::VpcHosting, Feature::ConfigurableRetention,
            Feature::AutonomousFineTuning,
        ];
        for feat in all {
            let min = feat.min_plan();
            // The min plan itself must grant the feature.
            let flags = FeatureFlags::for_plan(min);
            assert!(
                feat.check(&flags),
                "feature {:?}: min_plan() is {:?} but for_plan({:?}) does not enable it",
                feat, min, min,
            );
            // Any plan below must not.
            for lower in [Plan::Community, Plan::Professional, Plan::Enterprise] {
                if !lower.meets(min) {
                    let lf = FeatureFlags::for_plan(lower);
                    assert!(
                        !feat.check(&lf),
                        "feature {:?}: {:?} should NOT grant it (min={:?})",
                        feat, lower, min,
                    );
                }
            }
        }
    }
}
