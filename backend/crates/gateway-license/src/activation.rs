use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::{
    client::{LicenciaClient, ValidateRequest, ValidateResponse},
    error::LicenseError,
    features::{FeatureFlags, Plan},
    fingerprint,
    validator::LicenseValidator,
};

/// Current activation state of this gateway instance.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ActivationState {
    /// No license — running in Community mode.
    Unlicensed,
    /// License validated offline (signed JWT).
    OfflineValid {
        plan: String,
        expires_at: Option<String>,
        features: FeatureFlags,
    },
    /// License validated online with licencia platform.
    OnlineValid {
        license_id: String,
        plan: String,
        expires_at: Option<String>,
        hardware_fingerprint: String,
        features: FeatureFlags,
        next_check_in_secs: u64,
    },
    /// License expired but within grace period.
    GracePeriod {
        plan: String,
        expired_at: String,
        grace_until: String,
        features: FeatureFlags,
    },
    /// License invalid or expired past grace period.
    Invalid {
        reason: String,
    },
}

/// Manages license activation lifecycle: validation, heartbeat, grace period.
pub struct ActivationService {
    state: Arc<RwLock<ActivationState>>,
    licencia_client: Option<Arc<LicenciaClient>>,
    offline_validator: Option<Arc<LicenseValidator>>,
    license_key: Option<String>,
    instance_id: Option<String>,
    grace_period_days: i64,
}

impl ActivationService {
    pub fn new(
        licencia_client: Option<Arc<LicenciaClient>>,
        offline_validator: Option<Arc<LicenseValidator>>,
        license_key: Option<String>,
        instance_id: Option<String>,
        grace_period_days: i64,
    ) -> Self {
        Self {
            state: Arc::new(RwLock::new(ActivationState::Unlicensed)),
            licencia_client,
            offline_validator,
            license_key,
            instance_id,
            grace_period_days,
        }
    }

    /// Get the current activation state.
    pub async fn state(&self) -> ActivationState {
        self.state.read().await.clone()
    }

    /// Get the current feature flags based on activation state.
    pub async fn features(&self) -> FeatureFlags {
        let state = self.state.read().await;
        match &*state {
            ActivationState::OfflineValid { features, .. }
            | ActivationState::OnlineValid { features, .. }
            | ActivationState::GracePeriod { features, .. } => features.clone(),
            _ => FeatureFlags::for_plan(Plan::Community),
        }
    }

    /// Attempt to activate the license. Tries online first, falls back to offline.
    pub async fn activate(&self) -> Result<FeatureFlags, LicenseError> {
        let key = match &self.license_key {
            Some(k) => k.clone(),
            None => {
                info!("No license key configured — running in Community mode");
                let features = FeatureFlags::for_plan(Plan::Community);
                *self.state.write().await = ActivationState::Unlicensed;
                return Ok(features);
            }
        };

        // Try online activation first
        if let Some(ref client) = self.licencia_client {
            match self.try_online_activation(client, &key).await {
                Ok(features) => return Ok(features),
                Err(e) => {
                    warn!("Online activation failed: {e}. Trying offline validation...");
                }
            }
        }

        // Fall back to offline validation
        if let Some(ref validator) = self.offline_validator {
            match validator.validate(&key) {
                Ok(features) => {
                    let plan = format!("{:?}", features.plan).to_lowercase();
                    info!(plan = %plan, "License validated offline");
                    *self.state.write().await = ActivationState::OfflineValid {
                        plan,
                        expires_at: None,
                        features: features.clone(),
                    };
                    return Ok(features);
                }
                Err(LicenseError::Expired) => {
                    warn!("License expired — entering grace period");
                    let features = FeatureFlags::for_plan(Plan::Community);
                    *self.state.write().await = ActivationState::GracePeriod {
                        plan: "expired".to_string(),
                        expired_at: chrono::Utc::now().to_rfc3339(),
                        grace_until: (chrono::Utc::now() + chrono::Duration::days(self.grace_period_days)).to_rfc3339(),
                        features: features.clone(),
                    };
                    return Ok(features);
                }
                Err(e) => {
                    warn!("Offline validation failed: {e}");
                }
            }
        }

        // No valid license
        let features = FeatureFlags::for_plan(Plan::Community);
        *self.state.write().await = ActivationState::Invalid {
            reason: "License validation failed".to_string(),
        };
        Ok(features)
    }

    async fn try_online_activation(
        &self,
        client: &LicenciaClient,
        key: &str,
    ) -> Result<FeatureFlags, LicenseError> {
        let hw_fingerprint = fingerprint::generate_fingerprint(self.instance_id.as_deref());

        let request = ValidateRequest {
            key: key.to_string(),
            hardware_id: Some(hw_fingerprint.clone()),
            device_name: Some(hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "sentinel-gateway".to_string())),
            os: Some(std::env::consts::OS.to_string()),
            app_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        };

        let response = client.validate(request).await?;

        // Map entitlements to features
        let features = self.entitlements_to_features(&response);
        let plan = response.entitlements
            .as_ref()
            .and_then(|e| e.get("plan"))
            .and_then(|p| p.as_str())
            .unwrap_or("community")
            .to_string();

        let next_check = response.next_check_in.unwrap_or(3600);

        info!(
            license_id = ?response.license_id,
            plan = %plan,
            next_check_in = next_check,
            "License activated online"
        );

        *self.state.write().await = ActivationState::OnlineValid {
            license_id: response.license_id.unwrap_or_default(),
            plan,
            expires_at: response.expires_at,
            hardware_fingerprint: hw_fingerprint,
            features: features.clone(),
            next_check_in_secs: next_check,
        };

        Ok(features)
    }

    fn entitlements_to_features(&self, response: &ValidateResponse) -> FeatureFlags {
        let entitlements = response.entitlements.as_ref();

        let plan_str = entitlements
            .and_then(|e| e.get("plan"))
            .and_then(|p| p.as_str())
            .unwrap_or("community");

        let plan = Plan::from_str(plan_str);
        let mut features = FeatureFlags::for_plan(plan);

        // Override with specific entitlements from licencia
        if let Some(ent) = entitlements {
            if let Some(v) = ent.get("max_backends").and_then(|v| v.as_u64()) {
                features.max_backends = v as u32;
            }
            if let Some(v) = ent.get("max_users").and_then(|v| v.as_u64()) {
                features.max_users = v as u32;
            }
            if let Some(v) = ent.get("sso_enabled").and_then(|v| v.as_bool()) {
                features.sso_enabled = v;
            }
            if let Some(v) = ent.get("grpc_enabled").and_then(|v| v.as_bool()) {
                features.grpc_enabled = v;
            }
            if let Some(v) = ent.get("multi_tenant").and_then(|v| v.as_bool()) {
                features.multi_tenant = v;
            }
        }

        features
    }

    /// Periodic heartbeat — re-validate the license online.
    /// Call this from a background task at `next_check_in_secs` intervals.
    pub async fn heartbeat(&self) {
        let current = self.state.read().await.clone();
        match current {
            ActivationState::OnlineValid { .. } => {
                if let Err(e) = self.activate().await {
                    error!("License heartbeat failed: {e}");
                }
            }
            _ => {} // Only heartbeat for online-activated licenses
        }
    }
}
