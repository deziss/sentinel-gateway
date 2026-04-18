//! Field-level encryption using ChaCha20-Poly1305.
//!
//! Used to encrypt sensitive data at rest (e.g., backend credentials, API secrets).
//! The encryption key is a 32-byte hex string from configuration.

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Invalid encryption key: must be 32 bytes (64 hex chars)")]
    InvalidKey,
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    #[error("Invalid ciphertext format")]
    InvalidFormat,
}

/// Encrypts and decrypts field values using ChaCha20-Poly1305.
#[derive(Clone)]
pub struct FieldEncryptor {
    cipher: ChaCha20Poly1305,
}

impl FieldEncryptor {
    /// Create a new encryptor from a 32-byte hex-encoded key.
    pub fn new(hex_key: &str) -> Result<Self, CryptoError> {
        let key_bytes = hex::decode(hex_key).map_err(|_| CryptoError::InvalidKey)?;
        if key_bytes.len() != 32 {
            return Err(CryptoError::InvalidKey);
        }

        let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes)
            .map_err(|_| CryptoError::InvalidKey)?;

        Ok(Self { cipher })
    }

    /// Encrypt plaintext. Returns base64-encoded string: `nonce_bytes || ciphertext`.
    pub fn encrypt(&self, plaintext: &str) -> Result<String, CryptoError> {
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self.cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        // Prepend nonce to ciphertext
        let mut combined = Vec::with_capacity(12 + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        Ok(B64.encode(&combined))
    }

    /// Decrypt a base64-encoded ciphertext produced by `encrypt`.
    pub fn decrypt(&self, encoded: &str) -> Result<String, CryptoError> {
        let combined = B64.decode(encoded).map_err(|_| CryptoError::InvalidFormat)?;

        if combined.len() < 13 {
            return Err(CryptoError::InvalidFormat);
        }

        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;

        String::from_utf8(plaintext)
            .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> String {
        // 32 bytes = 64 hex chars
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string()
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let enc = FieldEncryptor::new(&test_key()).unwrap();
        let plaintext = "sk-abc123-super-secret-api-key";
        let ciphertext = enc.encrypt(plaintext).unwrap();
        assert_ne!(ciphertext, plaintext);
        let decrypted = enc.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn different_nonces_produce_different_ciphertext() {
        let enc = FieldEncryptor::new(&test_key()).unwrap();
        let c1 = enc.encrypt("test").unwrap();
        let c2 = enc.encrypt("test").unwrap();
        assert_ne!(c1, c2); // Random nonce → different output
    }

    #[test]
    fn invalid_key_rejected() {
        assert!(FieldEncryptor::new("too-short").is_err());
        assert!(FieldEncryptor::new("").is_err());
    }
}
