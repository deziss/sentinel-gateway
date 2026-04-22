use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tracing::{error, info};

use crate::client::{LicenciaClient, UsageReportRequest, UsageRecordPayload};
use gateway_db::repository::{UsageRecordRepository, TenantLicenseRepository};

/// Service for aggregating and reporting tenant usage to the Licencia platform.
pub struct UsageReporter {
    usage_repo: Arc<UsageRecordRepository>,
    license_repo: Arc<TenantLicenseRepository>,
    licencia_client: Arc<LicenciaClient>,
}

impl UsageReporter {
    pub fn new(
        usage_repo: Arc<UsageRecordRepository>,
        license_repo: Arc<TenantLicenseRepository>,
        licencia_client: Arc<LicenciaClient>,
    ) -> Self {
        Self {
            usage_repo,
            license_repo,
            licencia_client,
        }
    }

    /// Run a reporting pass for all active online licenses.
    /// Iterates through tenants with online licenses and pushes pending usage records.
    pub async fn report_all(&self) -> Result<(), anyhow::Error> {
        let licenses = self.license_repo.list_for_usage_report().await?;
        
        for license in licenses {
            if let Err(e) = self.report_for_tenant(license.tenant_id, &license.license_key, license.last_reported_at).await {
                error!("Failed to report usage for tenant {}: {}", license.tenant_id, e);
            }
        }
        
        Ok(())
    }

    /// Aggregates and reports usage for a specific tenant since their last reporting timestamp.
    async fn report_for_tenant(
        &self, 
        tenant_id: Uuid, 
        license_key: &str, 
        last_reported_at: Option<DateTime<Utc>>
    ) -> Result<(), anyhow::Error> {
        // Default to last 30 days if never reported
        let since = last_reported_at.unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));
        let end_time = Utc::now();
        
        // Fetch grouped aggregates from the usage table
        let aggregates = self.usage_repo.get_unreported_usage(tenant_id, since).await?;
        if aggregates.is_empty() {
            return Ok(());
        }
        
        let records: Vec<UsageRecordPayload> = aggregates.into_iter().map(|a| UsageRecordPayload {
            model: a.model.unwrap_or_else(|| "unknown".to_string()),
            provider: a.provider,
            total_requests: a.total_requests,
            total_tokens_input: a.total_tokens_input,
            total_tokens_output: a.total_tokens_output,
            total_cost: a.total_cost,
            start_time: a.start_time,
            end_time: a.end_time,
        }).collect();
        
        let req = UsageReportRequest {
            license_key: license_key.to_string(),
            tenant_id: tenant_id.to_string(),
            records,
        };
        
        // Push to Licencia
        self.licencia_client.report_usage(req).await?;
        
        // Update last_reported_at in the license table to avoid duplicate reporting
        let update = gateway_db::models::tenant_license::UpdateTenantLicense {
            status: None,
            plan: None,
            entitlements: None,
            fingerprint: None,
            expires_at: None,
            last_validated_at: None,
            last_reported_at: Some(end_time),
            offline_token: None,
        };
        
        self.license_repo.update(tenant_id, update).await?;
        
        info!("Reported usage for tenant {} up to {}", tenant_id, end_time);
        Ok(())
    }
}
