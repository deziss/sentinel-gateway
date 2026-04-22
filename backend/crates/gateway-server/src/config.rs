use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub telemetry: TelemetryAppConfig,
    pub proxy: ProxyAppConfig,
    pub license: LicenseConfig,
    pub redis: RedisConfig,
    pub platform: PlatformConfig,
    /// Optional observability export targets (Langfuse / Helicone).
    /// Disabled by default — no data leaves the gateway unless configured.
    #[serde(default)]
    pub observability: crate::observability_export::ObservabilityExportConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    /// "local" (default), "paas", or "platform"
    pub deployment_mode: String,
    /// Secure secret used to bypass license checks in PaaS mode.
    /// Should be a SHA-256 hash derived from the instance_id.
    pub developer_secret: Option<String>,
    pub saas_mode: bool,
    pub default_tenant_slug: Option<String>,
    pub max_body_size: usize,
    /// Unique identifier for this installation (auto-generated UUID if not set)
    pub instance_id: Option<String>,
    /// Human-readable name for this instance
    pub instance_name: Option<String>,
    /// 32-byte hex key for field encryption (ChaCha20-Poly1305)
    pub encryption_key: Option<String>,
    /// Allow all CORS origins (default true for local dev)
    #[serde(default = "default_cors_allow_all")]
    pub cors_allow_all: bool,
    /// Allowed CORS origins (only used when cors_allow_all is false)
    #[serde(default, deserialize_with = "deserialize_list_or_string")]
    pub cors_origins: Vec<String>,
    /// Require TLS on all incoming requests. Returns 426 Upgrade Required for plaintext.
    /// Trusts X-Forwarded-Proto header from trusted proxies.
    #[serde(default)]
    pub require_tls: bool,
    /// Trust X-Forwarded-Proto when behind a load balancer/reverse proxy (e.g., Nginx, ALB).
    #[serde(default = "default_trust_forwarded_proto")]
    pub trust_forwarded_proto: bool,
}

fn default_trust_forwarded_proto() -> bool { true }

fn default_cors_allow_all() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    /// Run migrations automatically at startup. Disable in production when
    /// a dedicated migrator job handles schema changes. When multiple replicas
    /// start simultaneously, only one will run migrations (advisory lock).
    /// Default: true (convenient for local dev).
    #[serde(default = "default_auto_migrate")]
    pub auto_migrate: bool,
    /// Enable pg_stat_statements extension and slow-query endpoint.
    /// Requires the `pg_stat_statements` extension (typically requires superuser).
    /// Default: false (off by default for compatibility).
    #[serde(default)]
    pub enable_query_stats: bool,
    /// Log SQL queries at debug level (also emitted as tracing spans).
    /// Default: false.
    #[serde(default)]
    pub log_queries: bool,
}

fn default_auto_migrate() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_private_key_path: String,
    pub jwt_public_key_path: String,
    pub access_token_ttl_minutes: i64,
    pub refresh_token_ttl_days: i64,
    pub max_failed_logins: i32,
    pub lockout_duration_minutes: i64,
    pub api_key_cache_ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryAppConfig {
    pub otlp_endpoint: Option<String>,
    pub service_name: String,
    pub log_level: String,
    pub prometheus_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyAppConfig {
    pub default_timeout_ms: u64,
    pub connect_timeout_ms: u64,
    pub max_retries: u32,
    pub health_check_interval_secs: u64,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_recovery_secs: u64,
    pub pool_idle_timeout_secs: u64,
    pub pool_max_idle_per_host: usize,
    pub max_body_size: usize,
    pub graphql_max_depth: u32,
    pub websocket_enabled: bool,
    pub lb_strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseConfig {
    pub license_key: Option<String>,
    pub public_key_path: Option<String>,
    pub grace_period_days: i64,
    /// Licencia platform base URL for online validation
    pub licencia_url: Option<String>,
    /// Licencia API key for platform-level operations
    pub licencia_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: Option<String>,
}

/// Platform connection settings (for sync with the central platform).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    /// Platform API base URL (e.g., "https://api.sentinel.io")
    pub url: Option<String>,
    /// Platform API key (obtained after sync registration)
    pub api_key: Option<String>,
    /// How often to auto-sync with platform (seconds, default 3600)
    pub sync_interval_secs: u64,
    /// Whether to auto-sync in background
    pub auto_sync: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                deployment_mode: "local".to_string(),
                developer_secret: None,
                saas_mode: false,
                default_tenant_slug: None,
                max_body_size: 10 * 1024 * 1024,
                instance_id: None,
                instance_name: None,
                encryption_key: None,
                cors_allow_all: true,
                cors_origins: vec![],
                require_tls: false,
                trust_forwarded_proto: true,
            },
            database: DatabaseConfig {
                url: "postgres://sentinel:sentinel@localhost:5432/sentinel_gateway".to_string(),
                max_connections: 20,
                auto_migrate: true,
                enable_query_stats: false,
                log_queries: false,
            },
            auth: AuthConfig {
                jwt_private_key_path: "keys/private.pem".to_string(),
                jwt_public_key_path: "keys/public.pem".to_string(),
                access_token_ttl_minutes: 15,
                refresh_token_ttl_days: 7,
                max_failed_logins: 5,
                lockout_duration_minutes: 15,
                api_key_cache_ttl_secs: 300,
            },
            telemetry: TelemetryAppConfig {
                otlp_endpoint: None,
                service_name: "sentinel-gateway".to_string(),
                log_level: "info".to_string(),
                prometheus_enabled: true,
            },
            proxy: ProxyAppConfig {
                default_timeout_ms: 30_000,
                connect_timeout_ms: 5_000,
                max_retries: 3,
                health_check_interval_secs: 30,
                circuit_breaker_threshold: 5,
                circuit_breaker_recovery_secs: 60,
                pool_idle_timeout_secs: 90,
                pool_max_idle_per_host: 256,
                max_body_size: 10 * 1024 * 1024,
                graphql_max_depth: 10,
                websocket_enabled: true,
                lb_strategy: "round_robin".to_string(),
            },
            license: LicenseConfig {
                license_key: None,
                public_key_path: None,
                grace_period_days: 7,
                licencia_url: None,
                licencia_api_key: None,
            },
            redis: RedisConfig {
                url: None,
            },
            platform: PlatformConfig {
                url: None,
                api_key: None,
                sync_interval_secs: 3600,
                auto_sync: false,
            },
            observability: Default::default(),
        }
    }
}

pub fn load_config() -> anyhow::Result<AppConfig> {
    dotenvy::dotenv().ok();
    let cfg = config::Config::builder()
        .add_source(config::File::with_name("config/gateway").required(false))
        .add_source(config::Environment::with_prefix("GATEWAY").separator("__"))
        .build()?;

    cfg.try_deserialize::<AppConfig>()
        .map_err(|e| {
            eprintln!("Configuration error: {e}");
            e.into()
        })
}

fn deserialize_list_or_string<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Visitor, SeqAccess};
    use std::fmt;

    struct ListOrString;

    impl<'de> Visitor<'de> for ListOrString {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a sequence or a comma-separated string")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect())
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut res = Vec::new();
            while let Some(value) = seq.next_element()? {
                res.push(value);
            }
            Ok(res)
        }
    }

    deserializer.deserialize_any(ListOrString)
}
