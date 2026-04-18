use gateway_db::repository::SettingRepository;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::TenantError;

// ── Sync State ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SyncState {
    Unlinked,
    Pending {
        platform_url: String,
        registered_at: String,
    },
    Linked {
        platform_url: String,
        instance_id: String,
        platform_tenant_id: String,
        linked_at: String,
        last_sync_at: Option<String>,
    },
}

// ── Platform API types ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct PlatformRegistrationRequest {
    pub instance_id: String,
    pub instance_name: String,
    pub admin_email: String,
    pub current_plan: String,
    pub user_count: i64,
    pub backend_count: i64,
    pub api_key_count: i64,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlatformRegistrationResponse {
    pub platform_tenant_id: String,
    pub api_key: String,
    pub license_key: Option<String>,
    pub plan: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPushPayload {
    pub instance_id: String,
    pub user_count: i64,
    pub backend_count: i64,
    pub api_key_count: i64,
    pub total_requests_30d: i64,
    pub total_cost_30d: f64,
    pub version: String,
    pub uptime_secs: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPullResponse {
    pub license_key: Option<String>,
    pub plan: String,
    pub features: serde_json::Value,
    pub messages: Vec<String>,
}

// ── Settings keys ──────────────────────────────────────────────────────────

const KEY_STATE: &str = "sync_state";
const KEY_PLATFORM_URL: &str = "sync_platform_url";
const KEY_INSTANCE_ID: &str = "sync_instance_id";
const KEY_PLATFORM_TENANT_ID: &str = "sync_platform_tenant_id";
const KEY_API_KEY: &str = "sync_api_key";
const KEY_LINKED_AT: &str = "sync_linked_at";
const KEY_LAST_SYNC_AT: &str = "sync_last_sync_at";

// ── Platform Sync Service ──────────────────────────────────────────────────

pub struct PlatformSyncService {
    http_client: reqwest::Client,
    setting_repo: Arc<SettingRepository>,
    tenant_id: Uuid,
}

impl PlatformSyncService {
    pub fn new(setting_repo: Arc<SettingRepository>, tenant_id: Uuid) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            setting_repo,
            tenant_id,
        }
    }

    /// Read current sync state from settings table.
    pub async fn get_state(&self) -> Result<SyncState, TenantError> {
        let map = self.setting_repo
            .get_map(self.tenant_id)
            .await
            .map_err(|e| TenantError::Internal(e.to_string()))?;

        match map.get(KEY_STATE).map(|s| s.as_str()) {
            Some("linked") => Ok(SyncState::Linked {
                platform_url: map.get(KEY_PLATFORM_URL).cloned().unwrap_or_default(),
                instance_id: map.get(KEY_INSTANCE_ID).cloned().unwrap_or_default(),
                platform_tenant_id: map.get(KEY_PLATFORM_TENANT_ID).cloned().unwrap_or_default(),
                linked_at: map.get(KEY_LINKED_AT).cloned().unwrap_or_default(),
                last_sync_at: map.get(KEY_LAST_SYNC_AT).cloned(),
            }),
            Some("pending") => Ok(SyncState::Pending {
                platform_url: map.get(KEY_PLATFORM_URL).cloned().unwrap_or_default(),
                registered_at: map.get(KEY_LINKED_AT).cloned().unwrap_or_default(),
            }),
            _ => Ok(SyncState::Unlinked),
        }
    }

    /// Register this instance with the platform.
    pub async fn register(
        &self,
        platform_url: &str,
        request: PlatformRegistrationRequest,
    ) -> Result<PlatformRegistrationResponse, TenantError> {
        let url = format!("{}/api/v1/instances/register", platform_url.trim_end_matches('/'));

        let resp = self.http_client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| TenantError::Internal(format!("Platform request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(TenantError::Internal(format!("Platform returned {status}: {body}")));
        }

        let result: PlatformRegistrationResponse = resp.json().await
            .map_err(|e| TenantError::Internal(format!("Invalid platform response: {e}")))?;

        // Store sync state
        let now = chrono::Utc::now().to_rfc3339();
        self.set(KEY_STATE, "linked").await?;
        self.set(KEY_PLATFORM_URL, platform_url).await?;
        self.set(KEY_INSTANCE_ID, &request.instance_id).await?;
        self.set(KEY_PLATFORM_TENANT_ID, &result.platform_tenant_id).await?;
        self.set(KEY_API_KEY, &result.api_key).await?;
        self.set(KEY_LINKED_AT, &now).await?;

        Ok(result)
    }

    /// Push local stats to platform.
    pub async fn push(
        &self,
        platform_url: &str,
        api_key: &str,
        payload: SyncPushPayload,
    ) -> Result<(), TenantError> {
        let url = format!("{}/api/v1/instances/sync", platform_url.trim_end_matches('/'));

        let resp = self.http_client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&payload)
            .send()
            .await
            .map_err(|e| TenantError::Internal(format!("Sync push failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(TenantError::Internal(format!("Sync push returned {status}: {body}")));
        }

        let now = chrono::Utc::now().to_rfc3339();
        self.set(KEY_LAST_SYNC_AT, &now).await?;

        Ok(())
    }

    /// Pull license/config from platform.
    pub async fn pull(
        &self,
        platform_url: &str,
        api_key: &str,
    ) -> Result<SyncPullResponse, TenantError> {
        let url = format!("{}/api/v1/instances/license", platform_url.trim_end_matches('/'));

        let resp = self.http_client
            .get(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await
            .map_err(|e| TenantError::Internal(format!("Sync pull failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(TenantError::Internal(format!("Sync pull returned {status}: {body}")));
        }

        let result: SyncPullResponse = resp.json().await
            .map_err(|e| TenantError::Internal(format!("Invalid sync pull response: {e}")))?;

        let now = chrono::Utc::now().to_rfc3339();
        self.set(KEY_LAST_SYNC_AT, &now).await?;

        Ok(result)
    }

    /// Unlink from platform — clear all sync state, revert to local mode.
    pub async fn unlink(&self) -> Result<(), TenantError> {
        let keys = [
            KEY_STATE, KEY_PLATFORM_URL, KEY_INSTANCE_ID,
            KEY_PLATFORM_TENANT_ID, KEY_API_KEY, KEY_LINKED_AT, KEY_LAST_SYNC_AT,
        ];
        for key in keys {
            let _ = self.setting_repo.delete(self.tenant_id, key).await;
        }
        Ok(())
    }

    async fn set(&self, key: &str, value: &str) -> Result<(), TenantError> {
        self.setting_repo
            .upsert(self.tenant_id, key, value, key == KEY_API_KEY)
            .await
            .map_err(|e| TenantError::Internal(e.to_string()))?;
        Ok(())
    }
}
