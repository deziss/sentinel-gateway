use sha2::{Digest, Sha256};
use std::env;

/// Generate a hardware fingerprint for this deployment instance.
///
/// Combines hostname, OS info, and an optional instance ID to create a
/// deterministic identifier that ties a license to a specific machine.
/// The fingerprint is a SHA-256 hash of the combined values.
pub fn generate_fingerprint(instance_id: Option<&str>) -> String {
    let mut hasher = Sha256::new();

    // Hostname
    if let Ok(hostname) = hostname::get() {
        hasher.update(hostname.to_string_lossy().as_bytes());
    }

    // OS info
    hasher.update(env::consts::OS.as_bytes());
    hasher.update(env::consts::ARCH.as_bytes());

    // Instance ID (if provided via config or generated at first boot)
    if let Some(id) = instance_id {
        hasher.update(id.as_bytes());
    }

    // MAC address or machine-id as extra entropy (Linux)
    #[cfg(target_os = "linux")]
    if let Ok(machine_id) = std::fs::read_to_string("/etc/machine-id") {
        hasher.update(machine_id.trim().as_bytes());
    }

    hex::encode(hasher.finalize())
}

/// Verify that the current hardware fingerprint matches the expected one.
pub fn verify_fingerprint(expected: &str, instance_id: Option<&str>) -> bool {
    let current = generate_fingerprint(instance_id);
    current == expected
}
