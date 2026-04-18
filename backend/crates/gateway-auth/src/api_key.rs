use rand::Rng;
use sha2::{Digest, Sha256};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

/// Generates a secure API key and returns (plaintext_key, sha256_hash).
///
/// The plaintext is shown once to the user; only the hash is stored.
pub fn generate_api_key() -> (String, String) {
    let mut rng = rand::thread_rng();
    let raw: Vec<u8> = (0..32).map(|_| rng.gen::<u8>()).collect();
    let plaintext = format!("sg_{}", URL_SAFE_NO_PAD.encode(&raw));
    let hash = hash_api_key(&plaintext);
    (plaintext, hash)
}

/// SHA-256 hash of an API key for DB storage.
pub fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}

pub struct ApiKeyService;

impl ApiKeyService {
    pub fn generate() -> (String, String) {
        generate_api_key()
    }

    pub fn hash(key: &str) -> String {
        hash_api_key(key)
    }

    /// Extract the key from an Authorization: Bearer header or sk-... prefix
    pub fn extract_from_header(auth_header: &str) -> Option<&str> {
        auth_header.strip_prefix("Bearer ").or_else(|| {
            if auth_header.starts_with("sg_") {
                Some(auth_header)
            } else {
                None
            }
        })
    }
}
