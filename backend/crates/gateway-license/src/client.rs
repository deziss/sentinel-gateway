use reqwest::Client;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, warn};

use crate::error::LicenseError;

/// Request payload for POST /licenses/validate on the licencia platform.
#[derive(Debug, Serialize)]
pub struct ValidateRequest {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hardware_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
}

/// Response from POST /licenses/validate.
#[derive(Debug, Deserialize, Clone)]
pub struct ValidateResponse {
    pub valid: bool,
    pub license_id: Option<String>,
    pub fingerprint: Option<String>,
    #[serde(rename = "type")]
    pub license_type: Option<String>,
    pub expires_at: Option<String>,
    pub entitlements: Option<serde_json::Value>,
    pub next_check_in: Option<u64>,
}

/// Request payload for POST /licenses/deactivate.
#[derive(Debug, Serialize)]
pub struct DeactivateRequest {
    pub key: String,
    pub hardware_id: String,
}

/// Client for the licencia licensing platform.
///
/// Handles online license validation, activation, and deactivation
/// via REST API calls to the licencia backend.
pub struct LicenciaClient {
    http: Client,
    base_url: String,
    api_key: Option<String>,
}

impl LicenciaClient {
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
        }
    }

    /// Validate a license key online. Optionally activates on a hardware fingerprint.
    pub async fn validate(&self, request: ValidateRequest) -> Result<ValidateResponse, LicenseError> {
        let url = format!("{}/licenses/validate", self.base_url);

        let mut req = self.http.post(&url).json(&request);
        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }

        let resp = req.send().await
            .map_err(|e| LicenseError::Internal(format!("License server unreachable: {e}")))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(LicenseError::Invalid);
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(status = %status, "License validation failed: {body}");
            return Err(LicenseError::Internal(format!("License server returned {status}")));
        }

        let result: ValidateResponse = resp.json().await
            .map_err(|e| LicenseError::Internal(format!("Invalid license response: {e}")))?;

        if !result.valid {
            return Err(LicenseError::Invalid);
        }

        info!(license_id = ?result.license_id, "License validated successfully");
        Ok(result)
    }

    /// Deactivate a license on a specific hardware fingerprint.
    pub async fn deactivate(&self, key: &str, hardware_id: &str) -> Result<(), LicenseError> {
        let url = format!("{}/licenses/deactivate", self.base_url);

        let request = DeactivateRequest {
            key: key.to_string(),
            hardware_id: hardware_id.to_string(),
        };

        let mut req = self.http.post(&url).json(&request);
        if let Some(ref api_key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {api_key}"));
        }

        let resp = req.send().await
            .map_err(|e| LicenseError::Internal(format!("License server unreachable: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(LicenseError::Internal(format!("Deactivation failed: {body}")));
        }

        info!("License deactivated on hardware {hardware_id}");
        Ok(())
    }

    /// Check if the license server is reachable.
    pub async fn health_check(&self) -> bool {
        let url = format!("{}/health", self.base_url);
        self.http.get(&url).send().await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Report usage metrics for a specific license/tenant.
    pub async fn report_usage(&self, request: UsageReportRequest) -> Result<(), LicenseError> {
        let url = format!("{}/usage/report", self.base_url);

        let mut req = self.http.post(&url).json(&request);
        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {key}"));
        }

        let resp = req.send().await
            .map_err(|e| LicenseError::Internal(format!("License server unreachable: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(status = %status, "Usage report failed: {body}");
            return Err(LicenseError::Internal(format!("Usage report failed with {status}")));
        }

        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct UsageReportRequest {
    pub license_key: String,
    pub tenant_id: String,
    pub records: Vec<UsageRecordPayload>,
}

#[derive(Debug, Serialize)]
pub struct UsageRecordPayload {
    pub model: String,
    pub provider: String,
    pub total_requests: i64,
    pub total_tokens_input: i64,
    pub total_tokens_output: i64,
    pub total_cost: f64,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
}
