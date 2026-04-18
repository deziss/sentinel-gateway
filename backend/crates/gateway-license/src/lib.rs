pub mod error;
pub mod features;
pub mod validator;
pub mod fingerprint;
pub mod client;
pub mod activation;

pub use error::LicenseError;
pub use features::{DeploymentMode, FeatureFlags, Plan};
pub use validator::LicenseValidator;
pub use client::LicenciaClient;
pub use activation::{ActivationService, ActivationState};
pub use fingerprint::generate_fingerprint;
