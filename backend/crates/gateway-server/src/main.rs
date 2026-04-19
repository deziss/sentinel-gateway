mod config;
mod handlers;
mod routes;
mod state;
mod guardrails_build;
mod observability_export;

use anyhow::Result;
use axum::Router;
use clap::{Parser, Subcommand};
use gateway_db::{create_pool, create_pool_with_options};
use gateway_license::{ActivationService, DeploymentMode, FeatureFlags, LicenciaClient, Plan};
use gateway_policy::{BudgetEnforcer, IpFilter, RateLimiter, PolicyEngine};
use gateway_core::health::HealthChecker;
use gateway_telemetry::{Metrics, TelemetryConfig, init_telemetry, shutdown_telemetry};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tower_http::cors::CorsLayer;
use tracing::{error, info, warn};
use gateway_db::repository::{
    ApiKeyRepository, AuditLogRepository, BackendRepository, RouteRepository,
    SettingRepository, TenantRepository, UsageRecordRepository, UserRepository,
    WebhookEndpointRepository, WebhookFailureRepository, PromptRepository,
    GuardrailRuleRepository,
};
use gateway_db::models::user::{CreateUser, UserRole as DbUserRole};
use gateway_db::models::tenant::CreateTenant;
use gateway_audit::AuditService;
use gateway_auth::password::PasswordService;
use gateway_auth::TokenBlacklist;
use gateway_auth::ApiKeyCache;

use fred::interfaces::ClientLike;
use crate::{config::load_config, state::AppState};

#[derive(Parser)]
#[command(name = "sentinel-gateway", version, about = "Sentinel Enterprise API Gateway")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the API gateway server
    Serve,
    /// Run database migrations
    Migrate,
    /// Create the initial superadmin user
    CreateAdmin {
        #[arg(long)]
        email: String,
        #[arg(long)]
        password: String,
        #[arg(long, default_value = "default")]
        tenant_slug: String,
    },
    /// Validate a license key (offline or online)
    ValidateLicense {
        #[arg(long)]
        key: String,
    },
    /// Generate RSA key pair for JWT signing
    GenerateKeys {
        #[arg(long, default_value = "keys")]
        output_dir: String,
    },
    /// Reset a user's password
    ResetPassword {
        #[arg(long)]
        email: String,
        #[arg(long)]
        password: String,
        #[arg(long, default_value = "default")]
        tenant_slug: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = load_config()?;

    // Initialize telemetry
    init_telemetry(&TelemetryConfig {
        otlp_endpoint: cfg.telemetry.otlp_endpoint.clone(),
        service_name: cfg.telemetry.service_name.clone(),
        service_version: env!("CARGO_PKG_VERSION").to_string(),
        log_level: cfg.telemetry.log_level.clone(),
        prometheus_enabled: cfg.telemetry.prometheus_enabled,
    });

    let result = match cli.command {
        Command::Serve => serve(cfg).await,
        Command::Migrate => {
            info!("Running migrations...");
            let pool = create_pool(&cfg.database.url, cfg.database.max_connections).await?;
            sqlx::migrate!("../../migrations").run(&pool).await?;
            info!("Migrations complete");
            Ok(())
        }
        Command::CreateAdmin { email, password, tenant_slug } => {
            cmd_create_admin(&cfg, &email, &password, &tenant_slug).await
        }
        Command::ValidateLicense { key } => {
            cmd_validate_license(&cfg, &key).await
        }
        Command::GenerateKeys { output_dir } => {
            cmd_generate_keys(&output_dir)
        }
        Command::ResetPassword { email, password, tenant_slug } => {
            cmd_reset_password(&cfg, &email, &password, &tenant_slug).await
        }
    };

    shutdown_telemetry();
    result
}

// ── CLI: create-admin ──────────────────────────────────────────────────────

async fn cmd_create_admin(
    cfg: &config::AppConfig,
    email: &str,
    password: &str,
    tenant_slug: &str,
) -> Result<()> {
    info!("Creating admin user: {email} in tenant: {tenant_slug}");
    let pool = create_pool(&cfg.database.url, cfg.database.max_connections).await?;
    let tenant_repo = TenantRepository::new(pool.clone());
    let user_repo = UserRepository::new(pool.clone());

    sqlx::migrate!("../../migrations").run(&pool).await?;

    let tenant_id = match tenant_repo.find_by_slug(tenant_slug).await {
        Ok(tenant) => tenant.id,
        Err(_) => {
            info!("Tenant {tenant_slug} not found. Creating...");
            let input = CreateTenant {
                name: "Default Tenant".to_string(),
                slug: tenant_slug.to_string(),
                plan: "community".to_string(),
                max_users: i32::MAX,
                max_api_keys: i32::MAX,
                max_backends: i32::MAX,
            };
            tenant_repo.create(input).await?.id
        }
    };

    let password_hash = PasswordService::hash(password)?;
    let user_input = CreateUser {
        tenant_id,
        email: email.to_string(),
        password_hash,
        role: DbUserRole::SuperAdmin,
    };

    match user_repo.create(user_input).await {
        Ok(_) => info!("SuperAdmin user created successfully!"),
        Err(e) => error!("Failed to create user: {e}"),
    }

    Ok(())
}

async fn cmd_reset_password(
    cfg: &config::AppConfig,
    email: &str,
    password: &str,
    tenant_slug: &str,
) -> Result<()> {
    info!("Resetting password for user: {email} in tenant: {tenant_slug}");
    let pool = create_pool(&cfg.database.url, cfg.database.max_connections).await?;
    let tenant_repo = TenantRepository::new(pool.clone());
    let user_repo = UserRepository::new(pool.clone());

    let tenant = tenant_repo.find_by_slug(tenant_slug).await
        .map_err(|e| anyhow::anyhow!("Tenant {tenant_slug} not found: {e}"))?;

    let user = user_repo.find_by_email(email, tenant.id).await
        .map_err(|e| anyhow::anyhow!("User {email} not found: {e}"))?;

    let password_hash = PasswordService::hash(password)?;
    user_repo.update_password(user.id, password_hash).await?;

    info!("Password reset successfully for {email}!");
    Ok(())
}

// ── CLI: validate-license ──────────────────────────────────────────────────

async fn cmd_validate_license(cfg: &config::AppConfig, key: &str) -> Result<()> {
    info!("Validating license key...");

    // Try offline validation
    if let Some(ref pk_path) = cfg.license.public_key_path {
        match std::fs::read(pk_path) {
            Ok(pk) => {
                match gateway_license::LicenseValidator::new(&pk, cfg.license.grace_period_days) {
                    Ok(validator) => match validator.validate(key) {
                        Ok(features) => {
                            info!("License VALID (offline)");
                            info!("  Plan: {:?}", features.plan);
                            info!("  Max backends: {}", features.max_backends);
                            info!("  Max users: {}", features.max_users);
                            info!("  SSO: {}", features.sso_enabled);
                            info!("  gRPC: {}", features.grpc_enabled);
                            info!("  Multi-tenant: {}", features.multi_tenant);
                            return Ok(());
                        }
                        Err(e) => warn!("Offline validation failed: {e}"),
                    },
                    Err(e) => warn!("Cannot init validator: {e}"),
                }
            }
            Err(e) => warn!("Cannot read public key: {e}"),
        }
    }

    // Try online validation
    if let Some(ref platform_url) = cfg.platform.url {
        info!("Trying online validation against {platform_url}...");
        let client = LicenciaClient::new(platform_url, cfg.platform.api_key.clone());
        let request = gateway_license::client::ValidateRequest {
            key: key.to_string(),
            hardware_id: Some(gateway_license::generate_fingerprint(cfg.server.instance_id.as_deref())),
            device_name: None,
            os: Some(std::env::consts::OS.to_string()),
            app_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        };

        match client.validate(request).await {
            Ok(resp) => {
                info!("License VALID (online)");
                info!("  License ID: {:?}", resp.license_id);
                info!("  Type: {:?}", resp.license_type);
                info!("  Expires: {:?}", resp.expires_at);
                info!("  Entitlements: {:?}", resp.entitlements);
                return Ok(());
            }
            Err(e) => warn!("Online validation failed: {e}"),
        }
    }

    error!("License validation FAILED — no valid license found");
    Ok(())
}

// ── CLI: generate-keys ─────────────────────────────────────────────────────

fn cmd_generate_keys(output_dir: &str) -> Result<()> {
    use std::fs;
    use std::process::Command as SysCmd;

    fs::create_dir_all(output_dir)?;

    let private_path = format!("{output_dir}/private.pem");
    let public_path = format!("{output_dir}/public.pem");

    info!("Generating RSA 2048-bit key pair...");

    // Generate private key
    let status = SysCmd::new("openssl")
        .args(["genrsa", "-out", &private_path, "2048"])
        .status();

    match status {
        Ok(s) if s.success() => info!("Private key written to {private_path}"),
        _ => {
            error!("Failed to generate private key. Is openssl installed?");
            return Ok(());
        }
    }

    // Extract public key
    let status = SysCmd::new("openssl")
        .args(["rsa", "-in", &private_path, "-pubout", "-out", &public_path])
        .status();

    match status {
        Ok(s) if s.success() => info!("Public key written to {public_path}"),
        _ => {
            error!("Failed to extract public key.");
            return Ok(());
        }
    }

    info!("Key pair generated successfully!");
    info!("  Private: {private_path}");
    info!("  Public:  {public_path}");
    info!("Set these in config: auth.jwt_private_key_path / auth.jwt_public_key_path");

    Ok(())
}

// ── Serve ──────────────────────────────────────────────────────────────────

/// Run database migrations under a PostgreSQL advisory lock so concurrent
/// replicas serialize safely. Arbitrary 63-bit lock ID = "sentinel-migrate".
async fn run_migrations_with_lock(pool: &gateway_db::DbPool) -> Result<()> {
    const LOCK_ID: i64 = 0x53454E_54494E45; // "SENTINEL"

    let mut conn = pool.acquire().await?;

    // Blocking advisory lock — one replica at a time runs migrations.
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(LOCK_ID)
        .execute(&mut *conn)
        .await?;

    // Run migrations. Connection is held to keep the lock.
    let result = sqlx::migrate!("../../migrations").run(&mut *conn).await;

    // Always release the lock, even on migration failure.
    let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(LOCK_ID)
        .execute(&mut *conn)
        .await;

    result?;
    info!("Migrations applied");
    Ok(())
}

/// Enable `pg_stat_statements` extension if configured. Requires superuser or
/// a managed DB with it preinstalled. Safe to fail — just logs a warning.
async fn enable_query_stats_if_configured(pool: &gateway_db::DbPool, enabled: bool) {
    if !enabled {
        return;
    }
    match sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_stat_statements")
        .execute(pool)
        .await
    {
        Ok(_) => info!("pg_stat_statements enabled — use /admin/slow-queries to inspect"),
        Err(e) => warn!("pg_stat_statements could not be enabled (requires superuser): {e}"),
    }
}

/// Validate production configuration requirements before startup.
/// Returns an error if critical production settings are misconfigured.
fn validate_production_config(cfg: &config::AppConfig) -> Result<()> {
    if cfg.server.deployment_mode.to_lowercase() != "platform" {
        return Ok(());
    }

    let mut errors: Vec<String> = Vec::new();

    // Encryption key required for platform mode (encrypts backend credentials at rest)
    if cfg.server.encryption_key.as_deref().unwrap_or("").is_empty() {
        errors.push(
            "server.encryption_key is required in platform mode. \
            Generate with: openssl rand -hex 32".to_string()
        );
    } else if cfg.server.encryption_key.as_deref().unwrap_or("").len() != 64 {
        errors.push(
            "server.encryption_key must be 64 hex characters (32 bytes). \
            Generate with: openssl rand -hex 32".to_string()
        );
    }

    // CORS wildcard in platform mode is dangerous
    if cfg.server.cors_allow_all && cfg.server.cors_origins.is_empty() {
        errors.push(
            "server.cors_allow_all=true is not allowed in platform mode. \
            Configure cors_origins with explicit allowed origins".to_string()
        );
    }

    // TLS should be enforced in platform mode
    if !cfg.server.require_tls {
        warn!("server.require_tls=false in platform mode. Strongly recommended to enable.");
    }

    if !errors.is_empty() {
        error!("Production configuration validation failed:");
        for err in &errors {
            error!("  - {err}");
        }
        anyhow::bail!("Invalid production configuration — see errors above");
    }

    Ok(())
}

async fn serve(mut cfg: config::AppConfig) -> Result<()> {
    info!("Starting Sentinel Gateway v{}", env!("CARGO_PKG_VERSION"));

    // Validate production configuration BEFORE touching anything
    validate_production_config(&cfg)?;

    // Database pool with SQLx -> tracing integration.
    // Slow queries (>200ms) logged at WARN regardless of log_queries setting.
    let pool = create_pool_with_options(
        &cfg.database.url,
        cfg.database.max_connections,
        cfg.database.log_queries,
    ).await?;
    info!(log_queries = cfg.database.log_queries, "Database connected");

    // Auto-run migrations on startup (opt-in).
    // Production deployments should use a dedicated migrator job instead.
    // Uses pg_advisory_xact_lock to prevent concurrent replicas from racing.
    if cfg.database.auto_migrate {
        info!("auto_migrate=true — running migrations with advisory lock");
        run_migrations_with_lock(&pool).await?;
    } else {
        info!("auto_migrate=false — skipping migrations (use `gateway-server migrate` job)");
    }

    // Optionally enable pg_stat_statements for slow-query observability
    enable_query_stats_if_configured(&pool, cfg.database.enable_query_stats).await;

    // ── Determine deployment mode ──────────────────────────────────────────
    let deployment_mode = match cfg.server.deployment_mode.to_lowercase().as_str() {
        "platform" => DeploymentMode::Platform,
        _ => DeploymentMode::Local,
    };

    let features = match deployment_mode {
        DeploymentMode::Local => {
            cfg.server.saas_mode = true;
            cfg.server.default_tenant_slug = Some("local".to_string());
            info!("Running in Community edition (local mode, offline — all core features enabled)");
            FeatureFlags::for_plan(Plan::Community)
        }
        DeploymentMode::Platform => {
            let features = if let Some(ref license_key) = cfg.license.license_key {
                if let Some(ref pk_path) = cfg.license.public_key_path {
                    match std::fs::read(pk_path) {
                        Ok(pk) => {
                            match gateway_license::LicenseValidator::new(&pk, cfg.license.grace_period_days) {
                                Ok(validator) => match validator.validate(license_key) {
                                    Ok(f) => {
                                        info!("Running in Platform mode (plan: {:?})", f.plan);
                                        f
                                    }
                                    Err(e) => {
                                        warn!("License validation failed: {e}. Falling back to Community.");
                                        FeatureFlags::for_plan(Plan::Community)
                                    }
                                },
                                Err(e) => {
                                    warn!("License validator init failed: {e}. Falling back to Community.");
                                    FeatureFlags::for_plan(Plan::Community)
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Cannot read license public key: {e}. Falling back to Community.");
                            FeatureFlags::for_plan(Plan::Community)
                        }
                    }
                } else {
                    warn!("License key set but no public key path. Falling back to Community.");
                    FeatureFlags::for_plan(Plan::Community)
                }
            } else {
                info!("Running in Platform mode (no license — Community plan)");
                FeatureFlags::for_plan(Plan::Community)
            };
            features
        }
    };

    // ── Activation Service ──────────────────────────────────────────────────
    let licencia_client = cfg.platform.url.as_ref().map(|url| {
        Arc::new(LicenciaClient::new(url, cfg.platform.api_key.clone()))
    });

    let offline_validator = if let Some(ref pk_path) = cfg.license.public_key_path {
        std::fs::read(pk_path).ok().and_then(|pk| {
            gateway_license::LicenseValidator::new(&pk, cfg.license.grace_period_days).ok().map(Arc::new)
        })
    } else {
        None
    };

    let activation_service = Arc::new(ActivationService::new(
        licencia_client,
        offline_validator,
        cfg.license.license_key.clone(),
        cfg.server.instance_id.clone(),
        cfg.license.grace_period_days,
    ));

    match activation_service.activate().await {
        Ok(f) => info!(plan = ?f.plan, "License activation complete"),
        Err(e) => warn!("License activation failed: {e} — running Community"),
    }

    // ── Auto-provision local tenant ────────────────────────────────────────
    let tenant_repo = Arc::new(TenantRepository::new(pool.clone()));

    if deployment_mode == DeploymentMode::Local {
        let local_slug = cfg.server.default_tenant_slug.as_deref().unwrap_or("local");
        match tenant_repo.find_by_slug(local_slug).await {
            Ok(t) => info!("Local tenant '{}' exists (id: {})", t.slug, t.id),
            Err(_) => {
                info!("Auto-provisioning local tenant '{local_slug}'...");
                let name = cfg.server.instance_name.as_deref().unwrap_or("Local Gateway");
                let input = CreateTenant {
                    name: name.to_string(),
                    slug: local_slug.to_string(),
                    plan: "community".to_string(),
                    max_users: i32::MAX,
                    max_api_keys: i32::MAX,
                    max_backends: i32::MAX,
                };
                match tenant_repo.create(input).await {
                    Ok(t) => info!("Local tenant created (id: {})", t.id),
                    Err(e) => error!("Failed to create local tenant: {e}"),
                }
            }
        }

        if cfg.server.instance_id.is_none() {
            cfg.server.instance_id = Some(uuid::Uuid::new_v4().to_string());
            info!("Generated instance_id: {}", cfg.server.instance_id.as_ref().unwrap());
        }
    }

    // ── JWT service ────────────────────────────────────────────────────────
    let private_key = std::fs::read(&cfg.auth.jwt_private_key_path)
        .unwrap_or_else(|_| include_bytes!("../../../keys/private.pem.example").to_vec());
    let public_key = std::fs::read(&cfg.auth.jwt_public_key_path)
        .unwrap_or_else(|_| include_bytes!("../../../keys/public.pem.example").to_vec());

    let jwt = gateway_auth::JwtService::new(
        &private_key, &public_key,
        cfg.auth.access_token_ttl_minutes, cfg.auth.refresh_token_ttl_days,
    ).unwrap_or_else(|_| {
        panic!("Could not initialize JWT service. Provide valid RSA keys or run: sentinel-gateway generate-keys");
    });

    // ── Policy engines ─────────────────────────────────────────────────────
    // Rate limiter selection:
    //   - Redis: if GATEWAY__REDIS__URL is set (preferred for multi-replica)
    //   - In-memory: single-replica deployments only
    // Multi-replica deployments MUST use Redis for consistent rate limiting.
    // We detect replica count via GATEWAY__REPLICAS env var (hint from orchestrator).
    let replicas: u32 = std::env::var("GATEWAY__REPLICAS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    let rate_limiter = if let Some(redis_url) = &cfg.redis.url {
        info!("Initializing Redis rate limiter: {redis_url}");
        // Parse the configured URL. Previously this was a silent no-op — the
        // default client connects to 127.0.0.1:6379 regardless of env var, so
        // every rate-limited request (e.g., /auth/login) hung forever in a
        // reconnect loop when the user-supplied URL pointed elsewhere.
        let config = fred::types::RedisConfig::from_url(redis_url)
            .map_err(|e| anyhow::anyhow!("Invalid REDIS URL {redis_url}: {e}"))?;
        let client = fred::clients::RedisClient::new(config, None, None, None);
        let _ = client.connect();
        // Bounded wait — if Redis is truly unreachable we don't want startup
        // to block forever. After 5s we fall back to in-memory (with a warning)
        // rather than leaving the gateway in a half-initialized state.
        match tokio::time::timeout(Duration::from_secs(5), client.wait_for_connect()).await {
            Ok(Ok(())) => {
                info!("Connected to Redis at {redis_url}");
                Arc::new(RateLimiter::new_redis(client))
            }
            Ok(Err(e)) => {
                warn!("Redis connection failed ({e}); falling back to in-memory rate limiter");
                Arc::new(RateLimiter::new_in_memory())
            }
            Err(_) => {
                warn!("Redis connect timed out after 5s; falling back to in-memory rate limiter");
                Arc::new(RateLimiter::new_in_memory())
            }
        }
    } else if replicas > 1 {
        // Fatal config error: multi-replica without Redis = inconsistent rate limits
        error!(
            replicas = replicas,
            "Multi-replica deployment requires Redis for consistent rate limiting. \
             Set GATEWAY__REDIS__URL or reduce GATEWAY__REPLICAS to 1."
        );
        anyhow::bail!("Redis required when replicas > 1");
    } else {
        warn!("Using in-memory rate limiter (single-replica only — NOT safe for scale-out)");
        Arc::new(RateLimiter::new_in_memory())
    };

    let budget_enforcer = Arc::new(BudgetEnforcer::new());
    let ip_filter = Arc::new(IpFilter::new());
    let metrics = Arc::new(Metrics::new());

    let password_service = Arc::new(PasswordService::new(
        rate_limiter.clone(),
        cfg.auth.max_failed_logins as u32,
        cfg.auth.lockout_duration_minutes as u32,
    ));

    let token_blacklist = Arc::new(TokenBlacklist::new());
    let api_key_cache = Arc::new(ApiKeyCache::new(
        Duration::from_secs(cfg.auth.api_key_cache_ttl_secs),
    ));

    let policy_engine = Arc::new(PolicyEngine::new(
        ip_filter, budget_enforcer.clone(), rate_limiter.clone(),
    ));

    let health_checker = Arc::new(HealthChecker::new());

    let proxy_engine = gateway_core::proxy::ProxyEngine::new(
        gateway_core::proxy::ProxyConfig {
            default_timeout_ms: cfg.proxy.default_timeout_ms,
            connect_timeout_ms: cfg.proxy.connect_timeout_ms,
            max_retries: cfg.proxy.max_retries,
            follow_redirects: true,
            pool_idle_timeout_secs: cfg.proxy.pool_idle_timeout_secs,
            pool_max_idle_per_host: cfg.proxy.pool_max_idle_per_host,
            max_body_size: cfg.proxy.max_body_size,
        }
    );

    let circuit_breaker = Arc::new(gateway_core::circuit_breaker::CircuitBreaker::new(
        cfg.proxy.circuit_breaker_threshold, cfg.proxy.circuit_breaker_recovery_secs,
    ));
    let lb_strategy = gateway_core::LbStrategy::from_str(&cfg.proxy.lb_strategy);
    // Inference-metrics cache is created unconditionally but only populated
    // when a backend exposes a `/metrics` endpoint. LoadBalancer falls back
    // to least-connections for backends without fresh metrics.
    let inference_metrics_cache = gateway_core::InferenceMetricsCache::new(
        std::time::Duration::from_secs(cfg.proxy.health_check_interval_secs * 3),
    );
    let load_balancer = gateway_core::LoadBalancer::new(lb_strategy)
        .with_inference_metrics(inference_metrics_cache.clone());

    let gateway_engine = Arc::new(gateway_core::GatewayEngine::new(
        proxy_engine, circuit_breaker, load_balancer, health_checker.clone(),
    ));

    // ── Repositories ───────────────────────────────────────────────────────
    let user_repo = Arc::new(UserRepository::new(pool.clone()));
    let api_key_repo = Arc::new(ApiKeyRepository::new(pool.clone()));
    let backend_repo = Arc::new(BackendRepository::new(pool.clone()));
    let route_repo = Arc::new(RouteRepository::new(pool.clone()));
    let audit_log_repo = Arc::new(AuditLogRepository::new(pool.clone()));
    let setting_repo = Arc::new(SettingRepository::new(pool.clone()));
    let usage_record_repo = Arc::new(UsageRecordRepository::new(pool.clone()));
    let webhook_repo = Arc::new(WebhookEndpointRepository::new(pool.clone()));
    let webhook_failure_repo = Arc::new(WebhookFailureRepository::new(pool.clone()));
    let prompt_repo = Arc::new(PromptRepository::new(pool.clone()));
    let guardrail_rule_repo = Arc::new(GuardrailRuleRepository::new(pool.clone()));
    // P0 feature-parity additions
    let team_repo = Arc::new(gateway_db::repository::TeamRepository::new(pool.clone()));
    let virtual_key_repo = Arc::new(gateway_db::repository::VirtualKeyRepository::new(pool.clone()));
    let llm_log_repo = Arc::new(gateway_db::repository::LlmLogRepository::new(pool.clone()));
    let tenant_pricing_repo = Arc::new(gateway_db::repository::TenantPricingRepository::new(pool.clone()));
    // SSO
    let sso_provider_repo = Arc::new(gateway_db::repository::SsoProviderRepository::new(pool.clone()));
    let sso_identity_repo = Arc::new(gateway_db::repository::SsoIdentityRepository::new(pool.clone()));
    let sso_auth_state_repo = Arc::new(gateway_db::repository::SsoAuthStateRepository::new(pool.clone()));
    let organization_repo = Arc::new(gateway_db::repository::OrganizationRepository::new(pool.clone()));
    let llm_feedback_repo = Arc::new(gateway_db::repository::LlmFeedbackRepository::new(pool.clone()));
    // Async write-behind LLM log service (batch=200, flush every 2s)
    let llm_log_service = gateway_audit::LlmLogService::start(llm_log_repo.clone(), 200, 2000);
    // Data-lake exporter (S3 / GCS / file) — no-op if DATA_LAKE_DESTINATION unset
    let data_lake = gateway_audit::DataLakeExporter::start(gateway_audit::DataLakeConfig::from_env());
    // Retention worker (runs every 24h; default 90 days unless LLM_LOG_RETENTION_DAYS env set)
    let default_retention_days = std::env::var("LLM_LOG_RETENTION_DAYS")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(90);
    gateway_audit::data_lake::spawn_retention_worker(
        llm_log_repo.clone(),
        setting_repo.clone(),
        tenant_repo.clone(),
        default_retention_days,
        24,
    );

    // ── Observability Export (optional) ────────────────────────────────────
    let observability_exporter = observability_export::ObservabilityExporter::start(
        cfg.observability.clone(),
    );

    // ── Services ───────────────────────────────────────────────────────────
    let audit_service = Arc::new(AuditService::start_with_webhooks(
        audit_log_repo.clone(), webhook_repo.clone(), 100, 5000, 3,
    ));

    let default_tenant_id = if let Some(ref slug) = cfg.server.default_tenant_slug {
        tenant_repo.find_by_slug(slug).await.ok().map(|t| t.id)
    } else {
        None
    };

    let tenant_service = Arc::new(gateway_tenant::service::TenantService::new(
        tenant_repo.clone(), default_tenant_id,
    ));

    let llm_router = Arc::new(gateway_llm::LlmRouter::new());
    // Simple non-semantic LLM response cache (exact-fingerprint; 1-day default)
    let llm_cache = Arc::new(gateway_llm::SemanticCache::new(86_400, 10_000));

    // ── MCP Server ─────────────────────────────────────────────────────────
    let mcp_registry = Arc::new(gateway_mcp::McpRegistry::default());
    let mcp_sessions = Arc::new(gateway_mcp::session::SessionStore::new(3600));
    let mcp_server = Arc::new(gateway_mcp::McpServer::new(mcp_registry, mcp_sessions));

    // ── Build AppState ─────────────────────────────────────────────────────
    let state = Arc::new(AppState {
        db: pool,
        jwt: Arc::new(jwt),
        token_blacklist: token_blacklist.clone(),
        api_key_cache: api_key_cache.clone(),
        features: Arc::new(features),
        policy_engine,
        gateway_engine,
        password_service,
        health_checker: health_checker.clone(),
        metrics,
        auth_config: cfg.auth.clone(),
        deployment_mode,
        server_config: cfg.server.clone(),
        platform_config: cfg.platform.clone(),
        tenant_repo: tenant_repo.clone(),
        user_repo,
        api_key_repo,
        backend_repo: backend_repo.clone(),
        route_repo,
        audit_log_repo,
        audit_service,
        tenant_service,
        setting_repo,
        activation_service: activation_service.clone(),
        webhook_repo,
        webhook_failure_repo: webhook_failure_repo.clone(),
        usage_record_repo,
        llm_router,
        llm_cache,
        mcp_server,
        prompt_repo,
        guardrail_rule_repo,
        team_repo,
        virtual_key_repo,
        llm_log_repo,
        tenant_pricing_repo,
        llm_log_service,
        sso_provider_repo,
        sso_identity_repo,
        sso_auth_state_repo,
        organization_repo,
        llm_feedback_repo,
        data_lake,
        observability_exporter,
    });

    // ── Background workers ─────────────────────────────────────────────────

    // Health checker
    let health_checker_task = health_checker.clone();
    let backend_repo_task = backend_repo.clone();
    let health_interval = Duration::from_secs(cfg.proxy.health_check_interval_secs);
    tokio::spawn(async move {
        info!("Starting active health checker worker");
        loop {
            if let Ok(backends) = backend_repo_task.list_all().await {
                health_checker_task.check_all(&backends).await;
            }
            tokio::time::sleep(health_interval).await;
        }
    });

    // Token blacklist cleanup (every 60s)
    let blacklist_task = token_blacklist.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            blacklist_task.cleanup();
        }
    });

    // API key cache cleanup (every 60s)
    let cache_task = api_key_cache.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            cache_task.cleanup();
        }
    });

    // Rate limiter cleanup (every 5 min)
    let rl_task = rate_limiter.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await;
            rl_task.cleanup(Duration::from_secs(300));
        }
    });

    // Budget stale period cleanup (every hour)
    let budget_task = budget_enforcer.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            budget_task.cleanup_stale();
        }
    });

    // License heartbeat (every hour)
    let heartbeat_svc = activation_service.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            heartbeat_svc.heartbeat().await;
        }
    });

    // Inference metrics scraper — polls /metrics on each active backend at the
    // health-check interval. Only does real work for LbStrategy::InferenceAware,
    // but runs unconditionally so operators can switch strategies without restart.
    {
        let backend_repo = backend_repo.clone();
        let metrics_cache = inference_metrics_cache.clone();
        let interval_secs = cfg.proxy.health_check_interval_secs;
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap_or_default();
            let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
            // Skip the immediate first tick
            ticker.tick().await;
            loop {
                ticker.tick().await;
                let backends = match backend_repo.list_all().await {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::debug!(error = %e, "Failed to list backends for metrics scrape");
                        continue;
                    }
                };
                for backend in backends.into_iter().filter(|b| b.is_active) {
                    let client = client.clone();
                    let cache = metrics_cache.clone();
                    let endpoint = backend.endpoint.clone();
                    let backend_id = backend.id;
                    tokio::spawn(async move {
                        if let Some(m) = gateway_core::inference_metrics::scrape_once(&client, &endpoint).await {
                            cache.set(backend_id, m);
                        }
                    });
                }
            }
        });
    }

    info!("Background workers started: health checker, blacklist cleanup, cache cleanup, rate limiter cleanup, budget cleanup, license heartbeat, inference metrics scraper");

    // ── Build Axum router ──────────────────────────────────────────────────
    // CORS: permissive by default for local dev, configurable for production
    let cors = if cfg.server.cors_allow_all {
        CorsLayer::permissive()
    } else {
        use axum::http::{HeaderValue, Method};
        use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
        let origins: Vec<HeaderValue> = cfg.server.cors_origins.iter()
            .filter_map(|o| o.parse::<HeaderValue>().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
            .allow_headers([AUTHORIZATION, CONTENT_TYPE,
                axum::http::HeaderName::from_static("x-tenant-id"),
                axum::http::HeaderName::from_static("x-api-key"),
                axum::http::HeaderName::from_static("x-csrf-token")])
            .allow_credentials(true)
    };

    let app = Router::new()
        .merge(routes::health::health_routes())
        .merge(routes::api_routes(state.clone()))
        .layer(axum::extract::DefaultBodyLimit::max(cfg.server.max_body_size))
        .layer(cors)
        // Security headers (always active)
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::HeaderName::from_static("x-frame-options"),
            axum::http::HeaderValue::from_static("DENY"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::HeaderName::from_static("x-content-type-options"),
            axum::http::HeaderValue::from_static("nosniff"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::HeaderName::from_static("x-xss-protection"),
            axum::http::HeaderValue::from_static("1; mode=block"),
        ))
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", cfg.server.host, cfg.server.port).parse()?;
    info!("Listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server stopped. Flushing telemetry...");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to listen for SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C, shutting down..."),
        _ = terminate => info!("Received SIGTERM, shutting down..."),
    }
}
