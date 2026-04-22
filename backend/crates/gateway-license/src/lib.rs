pub mod error;
pub mod features;
pub mod validator;
pub mod fingerprint;

#[cfg(feature = "saas")]
pub mod client;
#[cfg(feature = "saas")]
pub mod activation;
#[cfg(feature = "saas")]
pub mod tenant_license;
#[cfg(feature = "saas")]
pub mod usage_reporter;

pub use error::LicenseError;
pub use features::{DeploymentMode, Feature, FeatureFlags, Plan};
pub use validator::LicenseValidator;
pub use fingerprint::generate_fingerprint;

#[cfg(feature = "saas")]
pub use client::LicenciaClient;
#[cfg(feature = "saas")]
pub use activation::{ActivationService, ActivationState};
#[cfg(feature = "saas")]
pub use tenant_license::TenantLicenseService;
#[cfg(feature = "saas")]
pub use usage_reporter::UsageReporter;
